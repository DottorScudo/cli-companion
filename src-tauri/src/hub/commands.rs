//! Comandi Tauri dei thread dell'HUB. Validano gli argomenti per primi, fanno
//! l'I/O in `crate::blocking` e, a ogni modifica, emettono `hub-update` con il
//! riassunto aggiornato del thread toccato. Le letture funzionano anche con
//! l'HUB spento (non creano file); le scritture sono rifiutate.

use super::channel::Channel;
use super::files::threads_dir;
use super::model::{Author, Message, Thread, ThreadSummary};
use super::settings::is_enabled;
use super::state::HubState;
use super::threads::{summary, NewMessage, THREAD_NOT_FOUND};
use crate::store::data_dir;
use crate::workspaces::WorkspaceStore;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

/// Evento verso il frontend a ogni modifica di un thread (payload: `ThreadSummary`).
pub const HUB_UPDATE_EVENT: &str = "hub-update";

const HUB_DISABLED: &str = "L'Agents HUB è spento: attivalo nelle impostazioni.";
const CHANNEL_NOT_FOUND: &str = "canale non trovato: il workspace non esiste più";
const THREAD_ID_PREFIX: &str = "thr";
const MESSAGE_ID_PREFIX: &str = "msg";

#[tauri::command]
pub async fn hub_list_threads(app: AppHandle, channel: String) -> Result<Vec<ThreadSummary>, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let dir = dir(&app)?;
        ensure_channel(&app, &channel)?;
        Ok(app.state::<HubState>().read(&dir, &channel, |list| list.summaries()))
    })
    .await
}

#[tauri::command]
pub async fn hub_get_thread(app: AppHandle, channel: String, thread_id: String) -> Result<Thread, String> {
    let channel = Channel::parse(&channel)?;
    crate::blocking(move || {
        let dir = dir(&app)?;
        ensure_channel(&app, &channel)?;
        app.state::<HubState>()
            .read(&dir, &channel, |list| list.thread(&thread_id).cloned())
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
        let dir = dir(&app)?;
        let state = app.state::<HubState>();
        let now = chrono::Utc::now().timestamp_millis();
        let id = state.next_id(THREAD_ID_PREFIX, now);
        let thread = state.update(&dir, &channel, |list| {
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

/// Messaggio dell'utente in un thread; `reply_to` deve essere dello stesso thread.
#[tauri::command]
pub async fn hub_send(
    app: AppHandle,
    channel: String,
    thread_id: String,
    body: String,
    reply_to: Option<String>,
) -> Result<Message, String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    crate::blocking(move || {
        let dir = dir(&app)?;
        let state = app.state::<HubState>();
        let now = chrono::Utc::now().timestamp_millis();
        let new =
            NewMessage { id: state.next_id(MESSAGE_ID_PREFIX, now), author: Author::User, body, reply_to, event: None };
        let (message, touched) = state.update(&dir, &channel, |list| {
            ensure_channel(&app, &channel)?;
            let message = list.add_message(&thread_id, new, now)?.clone();
            let thread = list.thread(&thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
            Ok((message, summary(thread)))
        })?;
        emit_update(&app, &touched);
        Ok(message)
    })
    .await
}

fn ensure_enabled(app: &AppHandle) -> Result<(), String> {
    if is_enabled(app) {
        Ok(())
    } else {
        Err(HUB_DISABLED.to_string())
    }
}

/// Il canale di un workspace esiste finche' esiste il workspace: niente file
/// orfani per id inventati o workspace appena rimossi.
fn ensure_channel(app: &AppHandle, channel: &Channel) -> Result<(), String> {
    match channel {
        Channel::General => Ok(()),
        Channel::Workspace(id) if app.state::<WorkspaceStore>().find(id).is_some() => Ok(()),
        Channel::Workspace(_) => Err(CHANNEL_NOT_FOUND.to_string()),
    }
}

fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(threads_dir(&data_dir(app)?))
}

fn emit_update(app: &AppHandle, summary: &ThreadSummary) {
    if let Err(e) = app.emit(HUB_UPDATE_EVENT, summary) {
        log::error!("hub: evento {HUB_UPDATE_EVENT} non inviato: {e}");
    }
}
