//! I thread in memoria, gestiti da Tauri. Caricamento pigro: un canale si legge
//! dal disco la prima volta che serve e poi resta in memoria (con il suo
//! `JsonFile`) fino alla chiusura dell'app. Cosi' con l'HUB spento all'avvio
//! non si legge nulla, e la protezione contro la riscrittura di un file
//! illeggibile vale per tutti i salvataggi successivi.
//!
//! Un solo lock per tutti i canali, tenuto anche durante il salvataggio: le
//! modifiche arrivano dall'utente (e, piu' avanti, dagli eventi della board) e
//! sono rare; i comandi girano fuori dal main thread. I file degli allegati che
//! un salvataggio ha tolto (messaggi o thread potati) si cancellano dopo, fuori
//! dal lock, e solo se il salvataggio e' riuscito.

use super::attachments::{self, Garbage};
use super::channel::Channel;
use super::files::{self, HubDirs};
use super::threads::ChannelThreads;
use crate::store::JsonFile;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Prefissi degli id generati da `HubState::next_id`.
pub const THREAD_ID_PREFIX: &str = "thr";
pub const MESSAGE_ID_PREFIX: &str = "msg";
pub const ATTACHMENT_ID_PREFIX: &str = "att";

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

    /// Legge i thread del canale.
    pub fn read<R>(&self, dirs: &HubDirs, channel: &Channel, f: impl FnOnce(&ChannelThreads) -> R) -> R {
        let mut channels = self.lock();
        f(&loaded(&mut channels, &dirs.threads, channel).threads)
    }

    /// Applica `f` a una copia dei thread del canale, la salva e solo poi la
    /// pubblica in memoria: se `f` o il salvataggio falliscono nulla cambia.
    pub fn update<R>(
        &self,
        dirs: &HubDirs,
        channel: &Channel,
        f: impl FnOnce(&mut ChannelThreads) -> Result<R, String>,
    ) -> Result<R, String> {
        self.update_when(dirs, channel, f, |_| true)
    }

    /// Come `update`, ma salva e pubblica solo se `changed` lo dice dell'esito
    /// di `f`: un'operazione che non ha toccato nulla non riscrive (ne' crea) il file.
    pub fn update_when<R>(
        &self,
        dirs: &HubDirs,
        channel: &Channel,
        f: impl FnOnce(&mut ChannelThreads) -> Result<R, String>,
        changed: impl FnOnce(&R) -> bool,
    ) -> Result<R, String> {
        let (out, garbage) = {
            let mut channels = self.lock();
            let entry = loaded(&mut channels, &dirs.threads, channel);
            let mut next = entry.threads.clone();
            let out = f(&mut next)?;
            let mut garbage = Garbage::default();
            if changed(&out) {
                entry.file.save(&dirs.threads, &next)?;
                garbage = attachments::garbage(&entry.threads, &next);
                entry.threads = next;
            }
            (out, garbage)
        };
        if !garbage.is_empty() {
            attachments::discard(&dirs.attachments, channel, &garbage);
        }
        Ok(out)
    }

    /// Dimentica il canale e cancella il suo file e i suoi allegati (il
    /// workspace e' stato rimosso). Gli allegati si cancellano fuori dal lock.
    pub fn forget(&self, dirs: &HubDirs, channel: &Channel) {
        {
            let mut channels = self.lock();
            channels.remove(channel);
            files::delete(&dirs.threads, channel);
        }
        attachments::forget_channel(&dirs.attachments, channel);
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
    use crate::hub::model::{Attachment, Author, ThreadState};
    use crate::hub::threads::lifecycle::CLOSED_THREADS_MAX;
    use crate::hub::threads::{NewMessage, MESSAGES_MAX};
    use std::path::PathBuf;

    /// Cartella dati di prova (vuota) e le cartelle dell'HUB dentro.
    fn temp_dirs(name: &str) -> (PathBuf, HubDirs) {
        let d = std::env::temp_dir().join(format!("cc-hub-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dirs = HubDirs::in_data_dir(&d);
        (d, dirs)
    }

    fn ws() -> Channel {
        Channel::Workspace("ws-1".to_string())
    }

    fn create(state: &HubState, dirs: &HubDirs, channel: &Channel, id: &str) {
        state
            .update(dirs, channel, |list| list.create_thread(channel, id.to_string(), "Obiettivo", 1).map(|_| ()))
            .unwrap();
    }

    fn user(id: &str, body: &str) -> NewMessage {
        NewMessage { id: id.to_string(), author: Author::User, body: body.to_string(), reply_to: None, event: None }
    }

    /// Un allegato gia' nella cartella del thread, come dopo `Claimed::take`.
    fn stored(dirs: &HubDirs, thread_id: &str, id: &str) -> (Attachment, PathBuf) {
        let attachment =
            Attachment { id: id.to_string(), name: "nota.txt".to_string(), mime: "text/plain".to_string(), size: 2 };
        let path = attachments::stored_file(&dirs.attachments, &ws(), thread_id, &attachment).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "hi").unwrap();
        (attachment, path)
    }

    fn send_with(state: &HubState, dirs: &HubDirs, thread_id: &str, message_id: &str, attachment: Attachment) {
        state
            .update(dirs, &ws(), |l| {
                l.add_message_with_attachments(thread_id, user(message_id, ""), vec![attachment], 2).map(|_| ())
            })
            .unwrap();
    }

    fn add_texts(list: &mut ChannelThreads, thread_id: &str, count: usize) -> Result<(), String> {
        (0..count).try_for_each(|i| list.add_message(thread_id, user(&format!("m{i}"), "x"), 3).map(|_| ()))
    }

    #[test]
    fn reading_an_unknown_channel_creates_no_file() {
        let (data, dirs) = temp_dirs("read");
        let state = HubState::default();
        assert!(state.read(&dirs, &ws(), |l| l.threads.is_empty()));
        assert!(!data.exists());
    }

    #[test]
    fn threads_survive_a_restart_in_their_channel_file() {
        let (_, dirs) = temp_dirs("restart");
        let state = HubState::default();
        create(&state, &dirs, &ws(), "t1");
        create(&state, &dirs, &Channel::General, "g1");
        state.update(&dirs, &ws(), |l| l.add_message("t1", user("m1", "ciao"), 2).map(|_| ())).unwrap();
        assert!(dirs.threads.join("ws-1.json").is_file() && dirs.threads.join("general.json").is_file());
        let raw = std::fs::read_to_string(dirs.threads.join("ws-1.json")).unwrap();
        assert!(raw.contains("\"schema_version\": 1"), "{raw}");

        let restarted = HubState::default();
        restarted.read(&dirs, &ws(), |l| {
            assert_eq!(l.thread("t1").unwrap().messages[0].body, "ciao");
            assert!(l.thread("g1").is_none());
        });
        assert!(restarted.read(&dirs, &Channel::General, |l| l.thread("g1").is_some()));
    }

    #[test]
    fn a_file_written_before_attachments_existed_is_still_readable() {
        let (_, dirs) = temp_dirs("legacy");
        std::fs::create_dir_all(&dirs.threads).unwrap();
        let legacy = r#"{"threads":[{"id":"t1","channel":"ws-1","title":"Vecchio","state":"open",
            "created_at":1,"updated_at":2,"messages":[{"id":"m1","author":{"type":"user"},"kind":"text",
            "body":"ciao","created_at":2}]}]}"#;
        std::fs::write(dirs.threads.join("ws-1.json"), legacy).unwrap();
        HubState::default().read(&dirs, &ws(), |l| {
            let message = &l.thread("t1").unwrap().messages[0];
            assert_eq!(message.body, "ciao");
            assert!(message.attachments.is_empty());
        });
    }

    #[test]
    fn a_failed_update_changes_nothing() {
        let (data, dirs) = temp_dirs("failed");
        let state = HubState::default();
        let err = state.update(&dirs, &ws(), |_| Err::<(), _>("no".to_string())).unwrap_err();
        assert_eq!(err, "no");
        assert!(!data.exists());
    }

    #[test]
    fn an_update_that_changes_nothing_writes_nothing() {
        let (data, dirs) = temp_dirs("unchanged");
        let state = HubState::default();
        let out = state.update_when(&dirs, &ws(), |_| Ok(0), |n| *n > 0).unwrap();
        assert_eq!(out, 0);
        assert!(!data.exists());
        // Con `changed` falso anche le modifiche di `f` restano fuori.
        state
            .update_when(&dirs, &ws(), |l| l.create_thread(&ws(), "t1".to_string(), "x", 1).map(|_| ()), |_| false)
            .unwrap();
        assert!(state.read(&dirs, &ws(), |l| l.threads.is_empty()));
    }

    #[test]
    fn pruned_messages_lose_their_attachment_files() {
        let (_, dirs) = temp_dirs("prune-messages");
        let state = HubState::default();
        create(&state, &dirs, &ws(), "t1");
        let (first, first_path) = stored(&dirs, "t1", "att-1-1");
        send_with(&state, &dirs, "t1", "first", first);
        let (kept, kept_path) = stored(&dirs, "t1", "att-1-2");
        state
            .update(&dirs, &ws(), |l| {
                add_texts(l, "t1", MESSAGES_MAX - 1)?;
                l.add_message_with_attachments("t1", user("last", ""), vec![kept], 3).map(|_| ())
            })
            .unwrap();
        assert!(!first_path.parent().unwrap().exists());
        assert!(kept_path.is_file());
    }

    #[test]
    fn a_failed_save_deletes_no_attachment() {
        let (_, dirs) = temp_dirs("save-fails");
        let state = HubState::default();
        create(&state, &dirs, &ws(), "t1");
        let (attachment, path) = stored(&dirs, "t1", "att-1-1");
        send_with(&state, &dirs, "t1", "first", attachment);
        // Al posto della cartella dei thread un file: il salvataggio fallisce.
        std::fs::remove_dir_all(&dirs.threads).unwrap();
        std::fs::write(&dirs.threads, "").unwrap();
        assert!(state.update(&dirs, &ws(), |l| add_texts(l, "t1", MESSAGES_MAX)).is_err());
        assert!(path.is_file());
        assert!(state.read(&dirs, &ws(), |l| l.attachment("t1", "att-1-1").is_ok()));
    }

    #[test]
    fn pruned_closed_threads_lose_their_attachment_folder() {
        let (_, dirs) = temp_dirs("prune-threads");
        let state = HubState::default();
        create(&state, &dirs, &ws(), "old");
        let (attachment, path) = stored(&dirs, "old", "att-1-1");
        send_with(&state, &dirs, "old", "first", attachment);
        state
            .update(&dirs, &ws(), |l| {
                l.set_state("old", ThreadState::Closed, 3)?;
                for i in 0..CLOSED_THREADS_MAX {
                    let id = format!("t{i}");
                    l.create_thread(&ws(), id.clone(), "x", 4)?;
                    l.set_state(&id, ThreadState::Closed, 5 + i as i64)?;
                }
                Ok(())
            })
            .unwrap();
        assert!(state.read(&dirs, &ws(), |l| l.thread("old").is_none()));
        assert!(!path.exists() && !dirs.attachments.join("channels").join("ws-1").join("old").exists());
    }

    #[test]
    fn forgetting_a_channel_deletes_its_file_and_attachments_only() {
        let (_, dirs) = temp_dirs("forget");
        let state = HubState::default();
        create(&state, &dirs, &ws(), "t1");
        create(&state, &dirs, &Channel::General, "g1");
        let (attachment, path) = stored(&dirs, "t1", "att-1-1");
        send_with(&state, &dirs, "t1", "first", attachment);
        let general = dirs.attachments.join("channels").join("general");
        std::fs::create_dir_all(&general).unwrap();
        state.forget(&dirs, &ws());
        assert!(!dirs.threads.join("ws-1.json").exists() && dirs.threads.join("general.json").is_file());
        assert!(!path.exists() && !dirs.attachments.join("channels").join("ws-1").exists());
        assert!(general.is_dir());
        assert!(state.read(&dirs, &ws(), |l| l.threads.is_empty()));
        // Un canale mai salvato: nessun errore.
        state.forget(&dirs, &Channel::Workspace("ws-2".to_string()));
    }

    #[test]
    fn ids_are_unique_within_the_same_millisecond() {
        let state = HubState::default();
        assert_ne!(state.next_id("thr", 5), state.next_id("thr", 5));
        assert!(state.next_id("msg", 7).starts_with("msg-7-"));
    }
}
