//! I thread in memoria, gestiti da Tauri. Caricamento pigro: un canale si legge
//! dal disco la prima volta che serve e poi resta in memoria (con il suo
//! `JsonFile`) fino alla chiusura dell'app. Cosi' con l'HUB spento all'avvio
//! non si legge nulla, e la protezione contro la riscrittura di un file
//! illeggibile vale per tutti i salvataggi successivi.
//!
//! Un solo lock per tutti i canali, tenuto anche durante il salvataggio: le
//! modifiche arrivano dall'utente (e, piu' avanti, dagli eventi della board) e
//! sono rare; i comandi girano fuori dal main thread.

use super::channel::Channel;
use super::files;
use super::threads::ChannelThreads;
use crate::store::JsonFile;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Prefissi degli id generati da `HubState::next_id`.
pub const THREAD_ID_PREFIX: &str = "thr";
pub const MESSAGE_ID_PREFIX: &str = "msg";

struct Loaded {
    file: JsonFile<ChannelThreads>,
    threads: ChannelThreads,
}

#[derive(Default)]
pub struct HubState {
    channels: Mutex<HashMap<Channel, Loaded>>,
    sequence: AtomicU64,
}

impl HubState {
    /// Nuovo id `<prefisso>-<millisecondi>-<n>`: il contatore garantisce
    /// l'unicita' nello stesso millisecondo, i millisecondi tra un avvio e l'altro.
    pub fn next_id(&self, prefix: &str, now: i64) -> String {
        let n = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{now}-{n}")
    }

    /// Legge i thread del canale (`dir` = cartella dei thread).
    pub fn read<R>(&self, dir: &Path, channel: &Channel, f: impl FnOnce(&ChannelThreads) -> R) -> R {
        let mut channels = self.lock();
        f(&loaded(&mut channels, dir, channel).threads)
    }

    /// Applica `f` a una copia dei thread del canale, la salva e solo poi la
    /// pubblica in memoria: se `f` o il salvataggio falliscono nulla cambia.
    pub fn update<R>(
        &self,
        dir: &Path,
        channel: &Channel,
        f: impl FnOnce(&mut ChannelThreads) -> Result<R, String>,
    ) -> Result<R, String> {
        self.update_when(dir, channel, f, |_| true)
    }

    /// Come `update`, ma salva e pubblica solo se `changed` lo dice dell'esito
    /// di `f`: un'operazione che non ha toccato nulla non riscrive (ne' crea) il file.
    pub fn update_when<R>(
        &self,
        dir: &Path,
        channel: &Channel,
        f: impl FnOnce(&mut ChannelThreads) -> Result<R, String>,
        changed: impl FnOnce(&R) -> bool,
    ) -> Result<R, String> {
        let mut channels = self.lock();
        let entry = loaded(&mut channels, dir, channel);
        let mut next = entry.threads.clone();
        let out = f(&mut next)?;
        if changed(&out) {
            entry.file.save(dir, &next)?;
            entry.threads = next;
        }
        Ok(out)
    }

    /// Dimentica il canale e cancella il suo file (il workspace e' stato rimosso).
    pub fn forget(&self, dir: &Path, channel: &Channel) {
        let mut channels = self.lock();
        channels.remove(channel);
        files::delete(dir, channel);
    }

    /// Un panico isolato non deve fermare l'HUB: lo stato resta consistente
    /// perche' cambia solo dopo un salvataggio riuscito.
    fn lock(&self) -> MutexGuard<'_, HashMap<Channel, Loaded>> {
        self.channels.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn loaded<'a>(channels: &'a mut HashMap<Channel, Loaded>, dir: &Path, channel: &Channel) -> &'a mut Loaded {
    channels.entry(channel.clone()).or_insert_with(|| {
        let file = files::channel_file(channel);
        let threads = files::load(&file, dir, channel);
        Loaded { file, threads }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::model::Author;
    use crate::hub::threads::NewMessage;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-hub-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn ws() -> Channel {
        Channel::Workspace("ws-1".to_string())
    }

    fn create(state: &HubState, dir: &Path, channel: &Channel, id: &str) {
        state
            .update(dir, channel, |list| list.create_thread(channel, id.to_string(), "Obiettivo", 1).map(|_| ()))
            .unwrap();
    }

    #[test]
    fn reading_an_unknown_channel_creates_no_file() {
        let dir = temp_dir("read");
        let state = HubState::default();
        assert!(state.read(&dir, &ws(), |l| l.threads.is_empty()));
        assert!(!dir.exists());
    }

    #[test]
    fn threads_survive_a_restart_in_their_channel_file() {
        let dir = temp_dir("restart");
        let state = HubState::default();
        create(&state, &dir, &ws(), "t1");
        create(&state, &dir, &Channel::General, "g1");
        let msg = NewMessage {
            id: "m1".to_string(),
            author: Author::User,
            body: "ciao".to_string(),
            reply_to: None,
            event: None,
        };
        state.update(&dir, &ws(), |l| l.add_message("t1", msg, 2).map(|_| ())).unwrap();
        assert!(dir.join("ws-1.json").is_file() && dir.join("general.json").is_file());
        let raw = std::fs::read_to_string(dir.join("ws-1.json")).unwrap();
        assert!(raw.contains("\"schema_version\": 1"), "{raw}");

        let restarted = HubState::default();
        restarted.read(&dir, &ws(), |l| {
            assert_eq!(l.thread("t1").unwrap().messages[0].body, "ciao");
            assert!(l.thread("g1").is_none());
        });
        assert!(restarted.read(&dir, &Channel::General, |l| l.thread("g1").is_some()));
    }

    #[test]
    fn a_failed_update_changes_nothing() {
        let dir = temp_dir("failed");
        let state = HubState::default();
        let err = state.update(&dir, &ws(), |_| Err::<(), _>("no".to_string())).unwrap_err();
        assert_eq!(err, "no");
        assert!(!dir.exists());
    }

    #[test]
    fn an_update_that_changes_nothing_writes_nothing() {
        let dir = temp_dir("unchanged");
        let state = HubState::default();
        let out = state.update_when(&dir, &ws(), |_| Ok(0), |n| *n > 0).unwrap();
        assert_eq!(out, 0);
        assert!(!dir.exists());
        // Con `changed` falso anche le modifiche di `f` restano fuori.
        state
            .update_when(&dir, &ws(), |l| l.create_thread(&ws(), "t1".to_string(), "x", 1).map(|_| ()), |_| false)
            .unwrap();
        assert!(state.read(&dir, &ws(), |l| l.threads.is_empty()));
    }

    #[test]
    fn forgetting_a_channel_deletes_its_file_only() {
        let dir = temp_dir("forget");
        let state = HubState::default();
        create(&state, &dir, &ws(), "t1");
        create(&state, &dir, &Channel::General, "g1");
        state.forget(&dir, &ws());
        assert!(!dir.join("ws-1.json").exists() && dir.join("general.json").is_file());
        assert!(state.read(&dir, &ws(), |l| l.threads.is_empty()));
        // Un canale mai salvato: nessun errore.
        state.forget(&dir, &Channel::Workspace("ws-2".to_string()));
    }

    #[test]
    fn ids_are_unique_within_the_same_millisecond() {
        let state = HubState::default();
        assert_ne!(state.next_id("thr", 5), state.next_id("thr", 5));
        assert!(state.next_id("msg", 7).starts_with("msg-7-"));
    }
}
