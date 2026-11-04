//! I file dei thread: `<app_data>/threads/<workspace_id>.json` per ogni
//! workspace e `<app_data>/threads/general.json` per il Generale, nella busta
//! di `store` (migrazioni, backup dei file corrotti, scrittura atomica). Mai
//! nella repository dell'utente.

use super::channel::Channel;
use super::threads::ChannelThreads;
use crate::store::{adopt_envelope, JsonFile, Migration};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Cartella dei thread dentro la cartella dati dell'app.
const THREADS_DIR: &str = "threads";

/// Catena di migrazioni dei file dei thread: un passo in coda per ogni cambio di formato.
static STEPS: [Migration; 1] = [adopt_envelope];

pub fn threads_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(THREADS_DIR)
}

/// Il `JsonFile` del canale: va tenuto vivo finche' il canale e' in memoria,
/// perche' ricorda se il file su disco non si e' potuto leggere (e quindi non
/// va sovrascritto).
pub fn channel_file(channel: &Channel) -> JsonFile<ChannelThreads> {
    JsonFile::named(channel.file_name(), &STEPS)
}

/// Legge i thread del canale. File assente = nessun thread (e nessun file
/// creato); i thread di un altro canale (file spostato a mano) sono esclusi.
pub fn load(file: &JsonFile<ChannelThreads>, dir: &Path, channel: &Channel) -> ChannelThreads {
    let mut threads = file.load(dir).value;
    let foreign = threads.retain_channel(channel);
    if foreign > 0 {
        log::warn!("hub {}: {foreign} thread di un altro canale ignorati", file.name());
    }
    threads
}

/// Cancella il file dei thread del canale; un file gia' assente non e' un
/// errore. Gli altri errori sono nel log.
pub fn delete(dir: &Path, channel: &Channel) {
    let path = dir.join(channel.file_name());
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => log::error!("hub: impossibile eliminare {}: {e}", path.display()),
    }
}
