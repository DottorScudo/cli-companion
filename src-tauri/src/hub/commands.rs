//! Comandi Tauri dei thread dell'HUB. Validano gli argomenti per primi, fanno
//! l'I/O in `crate::blocking` e, a ogni modifica, emettono `hub-update` con il
//! riassunto aggiornato del thread toccato. Le letture funzionano anche con
//! l'HUB spento (non creano file); le scritture sono rifiutate.

use super::channel::Channel;
use super::files::HubDirs;
use super::model::{Message, Thread, ThreadSummary};
use super::send::{send_user_message, UserMessage};
use super::settings::is_enabled;
use super::state::{HubState, THREAD_ID_PREFIX};
use super::threads::{summary, THREAD_NOT_FOUND};
use crate::store::data_dir;
use crate::workspaces::WorkspaceStore;
use tauri::{AppHandle, Emitter, Manager};

/// Evento verso il frontend a ogni modifica di un thread (payload: `ThreadSummary`).
pub const HUB_UPDATE_EVENT: &str = "hub-update";

const HUB_DISABLED: &str = "L'Agents HUB è spento: attivalo nelle impostazioni.";
const CHANNEL_NOT_FOUND: &str = "canale non trovato: il workspace non esiste più";

#[tauri::command]
pub async fn hub_list_threads(app: AppHandle, channel: String) -> Result<Vec<ThreadSummary>, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let dirs = hub_dirs_of(&app)?;
        ensure_channel(&app, &channel)?;
        Ok(app.state::<HubState>().read(&dirs, &channel, |list| list.summaries()))
    })
    .await
}

#[tauri::command]
pub async fn hub_get_thread(app: AppHandle, channel: String, thread_id: String) -> Result<Thread, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let dirs = hub_dirs_of(&app)?;
        ensure_channel(&app, &channel)?;
        app.state::<HubState>()
            .read(&dirs, &channel, |list| list.thread(&thread_id).cloned())
            .ok_or_else(|| THREAD_NOT_FOUND.to_string())
    })
    .await
}

/// Thread libero aperto dall'utente (utile soprattutto nel Generale).
#[tauri::command]
pub async fn hub_create_thread(app: AppHandle, channel: String, title: String) -> Result<Thread, String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    crate::blocking(move || {
        let dirs = hub_dirs_of(&app)?;
        let state = app.state::<HubState>();
        let now = chrono::Utc::now().timestamp_millis();
        let id = state.next_id(THREAD_ID_PREFIX, now);
        let thread = state.update(&dirs, &channel, |list| {
            // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta
            // e poi cancella anche questo file.
            ensure_channel(&app, &channel)?;
            list.create_thread(&channel, id, &title, now).cloned()
        })?;
        emit_update(&app, &summary(&thread));
        Ok(thread)
    })
    .await
}

/// Messaggio dell'utente in un thread; `reply_to` deve essere dello stesso
/// thread. `attachments` sono gli id degli allegati gia' nello staging
/// (`hub_stage_attachment_*`; assente o vuoto = nessuno): con almeno un
/// allegato il testo puo' essere vuoto. L'invio e' quello di `send`: una risposta
/// a una domanda di un worker va al worker, altrimenti il motore dei segretari
/// decide chi svegliare e lavora in background.
#[tauri::command]
pub async fn hub_send(
    app: AppHandle,
    channel: String,
    thread_id: String,
    body: String,
    reply_to: Option<String>,
    attachments: Option<Vec<String>>,
) -> Result<Message, String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    let attachments = attachments.unwrap_or_default();
    crate::blocking(move || {
        let message = UserMessage { channel, thread_id, body, reply_to, attachment_ids: attachments, origin: None };
        send_user_message(&app, message)
    })
    .await
}

/// Il thread esiste ancora nel canale? Errore con l'HUB spento o la cartella dati
/// non disponibile; un canale sparito (workspace rimosso) risponde `false`. Per chi
/// lavora per conto dell'HUB (i connettori) senza conoscerne lo stato interno.
/// Bloccante: va chiamata fuori dal main thread.
pub fn thread_exists(app: &AppHandle, channel: &Channel, thread_id: &str) -> Result<bool, String> {
    ensure_enabled(app)?;
    if ensure_channel(app, channel).is_err() {
        return Ok(false);
    }
    let dirs = hub_dirs_of(app)?;
    Ok(app.state::<HubState>().read(&dirs, channel, |list| list.thread(thread_id).is_some()))
}

pub(super) fn ensure_enabled(app: &AppHandle) -> Result<(), String> {
    if is_enabled(app) {
        Ok(())
    } else {
        Err(HUB_DISABLED.to_string())
    }
}

/// Il canale di un workspace esiste finche' esiste il workspace: niente file
/// orfani per id inventati o workspace appena rimossi.
pub(super) fn ensure_channel(app: &AppHandle, channel: &Channel) -> Result<(), String> {
    match channel {
        Channel::General => Ok(()),
        Channel::Workspace(id) if app.state::<WorkspaceStore>().find(id).is_some() => Ok(()),
        Channel::Workspace(_) => Err(CHANNEL_NOT_FOUND.to_string()),
    }
}

/// Le cartelle dell'HUB (thread e allegati) nella cartella dati dell'app.
pub(super) fn hub_dirs_of(app: &AppHandle) -> Result<HubDirs, String> {
    Ok(HubDirs::in_data_dir(&data_dir(app)?))
}

pub(super) fn emit_update(app: &AppHandle, summary: &ThreadSummary) {
    if let Err(e) = app.emit(HUB_UPDATE_EVENT, summary) {
        log::error!("hub: evento {HUB_UPDATE_EVENT} non inviato: {e}");
    }
}
