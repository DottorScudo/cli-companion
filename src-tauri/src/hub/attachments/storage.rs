//! Le operazioni su disco degli allegati, nei percorsi di `layout`. Bloccanti:
//! vanno chiamate fuori dal main thread e, la copia nello staging, fuori dal
//! lock dell'HUB. Le cancellazioni non bloccano chi le chiede: un errore va nel
//! log, un file gia' assente non e' un errore.

use super::layout;
use super::{mime, name, too_large, ATTACHMENT_SIZE_MAX};
use crate::hub::channel::Channel;
use crate::hub::model::Attachment;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const NOT_STAGED: &str = "allegato non trovato o scaduto: caricalo di nuovo";
pub const NOT_FOUND: &str = "allegato non trovato";
/// Blocchi della copia nello staging.
const CHUNK: usize = 64 * 1024;

/// Copia nello staging il file `source` (scelto dall'utente con il dialog):
/// deve essere un file normale entro `ATTACHMENT_SIZE_MAX`.
pub fn stage_file(root: &Path, id: &str, source: &Path) -> Result<Attachment, String> {
    let raw_name = source.file_name().and_then(|n| n.to_str()).ok_or_else(|| "nome del file non valido".to_string())?;
    let name = name::clean(raw_name)?;
    let meta = fs::metadata(source).map_err(|e| format!("impossibile leggere «{name}»: {e}"))?;
    if !meta.is_file() {
        return Err(format!("«{name}» non è un file"));
    }
    if meta.len() > ATTACHMENT_SIZE_MAX {
        return Err(too_large(&name));
    }
    let file = File::open(source).map_err(|e| format!("impossibile aprire «{name}»: {e}"))?;
    stage(root, id, &name, file)
}

/// Mette nello staging i byte di un file senza percorso (immagine incollata,
/// file trascinato nella finestra).
pub fn stage_bytes(root: &Path, id: &str, raw_name: &str, bytes: &[u8]) -> Result<Attachment, String> {
    let name = name::clean(raw_name)?;
    if bytes.len() as u64 > ATTACHMENT_SIZE_MAX {
        return Err(too_large(&name));
    }
    stage(root, id, &name, bytes)
}

/// Scrive `source` in `staging/<id>/<name>`. Il limite vale anche per un file
/// che cresce durante la copia; se qualcosa fallisce la cartella sparisce.
fn stage(root: &Path, id: &str, name: &str, source: impl Read) -> Result<Attachment, String> {
    let dir = layout::staged_dir(root, id)?;
    fs::create_dir_all(&dir).map_err(|e| format!("impossibile preparare l'allegato: {e}"))?;
    let written = write_new(&dir.join(name), source);
    let outcome = match written {
        Ok((size, _)) if size > ATTACHMENT_SIZE_MAX => Err(too_large(name)),
        Ok((size, head)) => Ok(Attachment {
            id: id.to_string(),
            name: name.to_string(),
            mime: mime::detect(name, &head).to_string(),
            size,
        }),
        Err(e) => Err(format!("impossibile salvare «{name}»: {e}")),
    };
    if outcome.is_err() {
        remove_dir(&dir);
    }
    outcome
}

/// Copia fino a un byte oltre il limite (per accorgersi di un file troppo
/// grande senza leggerlo tutto) e restituisce i byte scritti e i primi `HEAD_LEN`.
fn write_new(path: &Path, source: impl Read) -> std::io::Result<(u64, Vec<u8>)> {
    let mut source = source.take(ATTACHMENT_SIZE_MAX + 1);
    let mut file = File::create_new(path)?;
    let mut buf = vec![0u8; CHUNK];
    let mut head = Vec::with_capacity(mime::HEAD_LEN);
    let mut size = 0u64;
    loop {
        let n = match source.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let missing = mime::HEAD_LEN.saturating_sub(head.len()).min(n);
        head.extend_from_slice(&buf[..missing]);
        file.write_all(&buf[..n])?;
        size += n as u64;
    }
    file.sync_all()?;
    Ok((size, head))
}

/// I metadati di un allegato nello staging, ricavati dal suo unico file.
pub fn staged(root: &Path, id: &str) -> Result<Attachment, String> {
    let dir = layout::staged_dir(root, id)?;
    let file = single_file(&dir).ok_or_else(|| NOT_STAGED.to_string())?;
    let name = file.file_name().and_then(|n| n.to_str()).ok_or_else(|| NOT_STAGED.to_string())?.to_string();
    if name::clean(&name).as_deref() != Ok(name.as_str()) {
        return Err(NOT_STAGED.to_string());
    }
    let size = fs::metadata(&file).map_err(|e| format!("impossibile leggere «{name}»: {e}"))?.len();
    if size > ATTACHMENT_SIZE_MAX {
        return Err(too_large(&name));
    }
    let head = read_head(&file).map_err(|e| format!("impossibile leggere «{name}»: {e}"))?;
    Ok(Attachment { id: id.to_string(), mime: mime::detect(&name, &head).to_string(), name, size })
}

fn single_file(dir: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(dir).ok()?;
    let first = entries.next()?.ok()?.path();
    (entries.next().is_none() && first.is_file()).then_some(first)
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::with_capacity(mime::HEAD_LEN);
    File::open(path)?.take(mime::HEAD_LEN as u64).read_to_end(&mut head)?;
    Ok(head)
}

/// Gli allegati spostati dallo staging nella cartella di un thread, prima di
/// salvare il messaggio. Se il salvataggio fallisce, `release` li riporta
/// nello staging: l'utente puo' riprovare con gli stessi id.
#[derive(Debug)]
pub struct Claimed {
    root: PathBuf,
    channel: Channel,
    thread_id: String,
    attachments: Vec<Attachment>,
}

impl Claimed {
    /// Sposta gli allegati `ids` dallo staging nel thread. Lo spostamento e'
    /// atomico: due invii con lo stesso id non possono prenderlo entrambi. Se un
    /// allegato manca, quelli gia' spostati tornano nello staging.
    pub fn take(root: &Path, channel: &Channel, thread_id: &str, ids: &[String]) -> Result<Self, String> {
        let mut claimed = Self {
            root: root.to_path_buf(),
            channel: channel.clone(),
            thread_id: thread_id.to_string(),
            attachments: Vec::new(),
        };
        if ids.is_empty() {
            return Ok(claimed);
        }
        let thread_dir = layout::thread_dir(root, channel, thread_id)?;
        fs::create_dir_all(&thread_dir).map_err(|e| format!("impossibile preparare gli allegati: {e}"))?;
        for id in ids {
            match claimed.move_in(id) {
                Ok(attachment) => claimed.attachments.push(attachment),
                Err(e) => {
                    claimed.release();
                    return Err(e);
                }
            }
        }
        Ok(claimed)
    }

    fn move_in(&self, id: &str) -> Result<Attachment, String> {
        let attachment = staged(&self.root, id)?;
        let from = layout::staged_dir(&self.root, id)?;
        let to = layout::stored_dir(&self.root, &self.channel, &self.thread_id, id)?;
        fs::rename(&from, &to).map_err(|e| match e.kind() {
            // Un altro invio l'ha appena preso, o lo staging e' stato ripulito.
            ErrorKind::NotFound => NOT_STAGED.to_string(),
            _ => format!("impossibile allegare «{}»: {e}", attachment.name),
        })?;
        Ok(attachment)
    }

    fn move_back(&self, id: &str) -> Result<(), String> {
        let from = layout::stored_dir(&self.root, &self.channel, &self.thread_id, id)?;
        let to = layout::staged_dir(&self.root, id)?;
        fs::rename(&from, &to).map_err(|e| e.to_string())
    }

    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// Riporta gli allegati nello staging (il messaggio non e' stato salvato).
    pub fn release(self) {
        for attachment in &self.attachments {
            if let Err(e) = self.move_back(&attachment.id) {
                log::error!("hub: allegato {} non riportato nello staging: {e}", attachment.id);
            }
        }
    }
}

/// Cancella i file che un salvataggio ha tolto (vedi `garbage`).
pub fn discard(root: &Path, channel: &Channel, garbage: &super::Garbage) {
    for thread_id in &garbage.threads {
        match layout::thread_dir(root, channel, thread_id) {
            Ok(dir) => remove_dir(&dir),
            Err(e) => log::warn!("hub: allegati del thread non eliminati: {e}"),
        }
    }
    for (thread_id, id) in &garbage.attachments {
        match layout::stored_dir(root, channel, thread_id, id) {
            Ok(dir) => remove_dir(&dir),
            Err(e) => log::warn!("hub: allegato non eliminato: {e}"),
        }
    }
}

/// Cancella tutti gli allegati del canale (il workspace e' stato rimosso).
pub fn forget_channel(root: &Path, channel: &Channel) {
    remove_dir(&layout::channel_dir(root, channel));
}

/// Toglie dallo staging gli allegati modificati prima di `cutoff`: caricati e
/// mai inviati (messaggio abbandonato, app chiusa). Staging assente: nulla.
pub fn purge_staging(root: &Path, cutoff: SystemTime) {
    let staging = layout::staging_dir(root);
    let entries = match fs::read_dir(&staging) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => {
            log::error!("hub: staging degli allegati illeggibile: {e}");
            return;
        }
    };
    let dated: Vec<(PathBuf, SystemTime)> = entries
        .filter_map(|entry| match entry.and_then(|e| Ok((e.path(), e.metadata()?.modified()?))) {
            Ok(dated) => Some(dated),
            Err(e) => {
                log::warn!("hub: voce dello staging illeggibile: {e}");
                None
            }
        })
        .collect();
    for path in stale(dated, cutoff) {
        remove_dir(&path);
    }
}

/// Le voci dello staging modificate prima di `cutoff`.
fn stale(entries: Vec<(PathBuf, SystemTime)>, cutoff: SystemTime) -> Vec<PathBuf> {
    entries.into_iter().filter(|(_, modified)| *modified < cutoff).map(|(path, _)| path).collect()
}

/// I byte di un allegato (per l'anteprima), entro `ATTACHMENT_SIZE_MAX`.
pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => NOT_FOUND.to_string(),
        _ => format!("impossibile leggere l'allegato: {e}"),
    })?;
    let mut bytes = Vec::new();
    file.take(ATTACHMENT_SIZE_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("impossibile leggere l'allegato: {e}"))?;
    if bytes.len() as u64 > ATTACHMENT_SIZE_MAX {
        return Err("l'allegato supera la dimensione consentita".to_string());
    }
    Ok(bytes)
}

fn remove_dir(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => log::error!("hub: impossibile eliminare {}: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests;
