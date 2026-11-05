//! Comandi Tauri degli allegati, il percorso assoluto di un allegato e la forma
//! dei percorsi per gli agenti di un canale, che usa anche il motore. Il
//! caricamento e' una scrittura (rifiutato con l'HUB spento, prima di creare
//! cartelle); lettura e percorso funzionano sempre. L'I/O e' in
//! `crate::blocking`, la copia nello staging senza il lock dell'HUB.

use super::listing::AttachmentPaths;
use super::storage::{self, NOT_FOUND};
use super::stored_file;
use super::{too_large, ATTACHMENT_SIZE_MAX};
use crate::hub::channel::Channel;
use crate::hub::commands::{ensure_channel, ensure_enabled, hub_dirs_of};
use crate::hub::files::HubDirs;
use crate::hub::model::Attachment;
use crate::hub::state::{HubState, ATTACHMENT_ID_PREFIX};
use crate::platform::TargetKind;
use crate::workspaces::WorkspaceStore;
use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager};

/// Header con il nome del file in `hub_stage_attachment_bytes`, codificato
/// come `encodeURIComponent` (gli header portano solo ASCII).
pub const FILE_NAME_HEADER: &str = "x-hub-file-name";
/// Dopo quanto un allegato caricato e mai inviato si toglie dallo staging.
const STAGING_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Carica nello staging il file scelto con il dialog (percorso assoluto).
#[tauri::command]
pub async fn hub_stage_attachment_file(app: AppHandle, path: String) -> Result<Attachment, String> {
    ensure_enabled(&app)?;
    let source = PathBuf::from(path);
    if !source.is_absolute() {
        return Err("percorso del file non valido".to_string());
    }
    crate::blocking(move || {
        let (root, id) = prepare_staging(&app)?;
        storage::stage_file(&root, &id, &source)
    })
    .await
}

/// Carica nello staging i byte di un file senza percorso (immagine incollata,
/// file trascinato): corpo raw della richiesta, nome nell'header `FILE_NAME_HEADER`.
#[tauri::command]
pub async fn hub_stage_attachment_bytes(app: AppHandle, request: Request<'_>) -> Result<Attachment, String> {
    ensure_enabled(&app)?;
    let name = file_name_from_header(request.headers().get(FILE_NAME_HEADER).and_then(|v| v.to_str().ok()))?;
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("contenuto dell'allegato mancante".to_string());
    };
    if bytes.len() as u64 > ATTACHMENT_SIZE_MAX {
        return Err(too_large(&name));
    }
    let bytes = bytes.clone();
    crate::blocking(move || {
        let (root, id) = prepare_staging(&app)?;
        storage::stage_bytes(&root, &id, &name, &bytes)
    })
    .await
}

/// I byte di un allegato, per l'anteprima (il frontend ne fa un Blob URL).
#[tauri::command]
pub async fn hub_read_attachment(
    app: AppHandle,
    channel: String,
    thread_id: String,
    attachment_id: String,
) -> Result<Response, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let path = attachment_path(&app, &channel, &thread_id, &attachment_id)?;
        Ok(Response::new(storage::read(&path)?))
    })
    .await
}

/// Il percorso assoluto di un allegato, per aprirlo con l'app predefinita.
#[tauri::command]
pub async fn hub_attachment_path(
    app: AppHandle,
    channel: String,
    thread_id: String,
    attachment_id: String,
) -> Result<String, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let path = attachment_path(&app, &channel, &thread_id, &attachment_id)?;
        path.into_os_string().into_string().map_err(|_| "percorso dell'allegato non rappresentabile".to_string())
    })
    .await
}

/// Il percorso assoluto dell'allegato `attachment_id` di un messaggio del
/// thread: solo un allegato che il thread nomina e il cui file esiste, sempre
/// dentro la cartella degli allegati. Bloccante.
pub fn attachment_path(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    attachment_id: &str,
) -> Result<PathBuf, String> {
    let dirs = hub_dirs_of(app)?;
    ensure_channel(app, channel)?;
    let attachment =
        app.state::<HubState>().read(&dirs, channel, |list| list.attachment(thread_id, attachment_id).cloned())?;
    existing_file(&dirs, channel, thread_id, &attachment)
}

/// Come vedono gli allegati del canale gli agenti che ci lavorano: dalla
/// radice `root`, nella forma dell'ambiente del workspace del canale (Windows
/// nel Generale, dove non girano worker, o se il workspace non c'e' piu').
pub fn agent_paths<'a>(app: &AppHandle, root: &'a Path, channel: &Channel) -> AttachmentPaths<'a> {
    let env = match channel {
        Channel::Workspace(id) => app.state::<WorkspaceStore>().find(id).map_or(TargetKind::Windows, |ws| ws.kind),
        Channel::General => TargetKind::Windows,
    };
    AttachmentPaths { root, env }
}

fn existing_file(
    dirs: &HubDirs,
    channel: &Channel,
    thread_id: &str,
    attachment: &Attachment,
) -> Result<PathBuf, String> {
    let path = stored_file(&dirs.attachments, channel, thread_id, attachment)?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(NOT_FOUND.to_string())
    }
}

/// Toglie dallo staging gli allegati di un avvio precedente (mai inviati: il
/// frontend li ha persi con la chiusura), in background. Con l'HUB spento lo
/// staging di solito non c'e' e non succede nulla; non crea cartelle.
pub fn purge_staging_in_background(app: &AppHandle) {
    let app = app.clone();
    let started = SystemTime::now();
    tauri::async_runtime::spawn_blocking(move || match hub_dirs_of(&app) {
        Ok(dirs) => storage::purge_staging(&dirs.attachments, started),
        Err(e) => log::error!("hub: staging degli allegati non ripulito: {e}"),
    });
}

/// La radice degli allegati e un id nuovo; intanto via lo staging scaduto.
fn prepare_staging(app: &AppHandle) -> Result<(PathBuf, String), String> {
    let root = hub_dirs_of(app)?.attachments;
    purge_expired(&root);
    let id = app.state::<HubState>().next_id(ATTACHMENT_ID_PREFIX, chrono::Utc::now().timestamp_millis());
    Ok((root, id))
}

fn purge_expired(root: &Path) {
    if let Some(cutoff) = SystemTime::now().checked_sub(STAGING_TTL) {
        storage::purge_staging(root, cutoff);
    }
}

/// Il nome del file dall'header: decodifica percent-encoding in UTF-8 (la
/// pulizia vera la fa `name::clean`).
fn file_name_from_header(raw: Option<&str>) -> Result<String, String> {
    let raw = raw.filter(|r| !r.is_empty()).ok_or_else(|| "nome del file mancante".to_string())?;
    percent_decode_str(raw)
        .decode_utf8()
        .map(|name| name.into_owned())
        .map_err(|_| "nome del file non valido".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_come_percent_encoded_in_the_header() {
        assert_eq!(file_name_from_header(Some("foto.png")).unwrap(), "foto.png");
        assert_eq!(file_name_from_header(Some("citt%C3%A0%20nuova.txt")).unwrap(), "città nuova.txt");
        assert_eq!(file_name_from_header(Some("..%2F..%2Fx")).unwrap(), "../../x");
        assert!(file_name_from_header(None).is_err());
        assert!(file_name_from_header(Some("")).is_err());
        assert!(file_name_from_header(Some("%FF%FE")).is_err());
    }
}
