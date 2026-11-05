//! Chiudere e riaprire un thread a mano: il comando con cui lo fa l'utente
//! (`hub_set_thread_state`) e l'operazione che usa anche il tool
//! `set_thread_state` dei segretari. Riaprire non sveglia nessuno e un
//! messaggio dell'utente in un thread chiuso si comporta come prima: cambia
//! solo lo stato salvato. Senza cambiamento non si riscrive il file ne' si
//! emette `hub-update`.

use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, ensure_enabled, hub_dirs_of};
use super::model::{ThreadState, ThreadSummary};
use super::state::HubState;
use super::threads::{summary, THREAD_NOT_FOUND};
use tauri::{AppHandle, Manager};

/// Il thread dopo la richiesta e se lo stato e' cambiato davvero.
pub struct StateChange {
    pub summary: ThreadSummary,
    pub changed: bool,
}

/// Chiude o riapre il thread `thread_id` (`closed`: `true` lo chiude).
#[tauri::command]
pub async fn hub_set_thread_state(
    app: AppHandle,
    channel: String,
    thread_id: String,
    closed: bool,
) -> Result<ThreadSummary, String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    let state = if closed { ThreadState::Closed } else { ThreadState::Open };
    crate::blocking(move || apply(&app, &channel, &thread_id, state).map(|change| change.summary)).await
}

/// Mette il thread in `state`, lo salva ed emette `hub-update` se e' cambiato.
/// Bloccante: va chiamata fuori dal main thread.
pub(super) fn apply(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    state: ThreadState,
) -> Result<StateChange, String> {
    let dirs = hub_dirs_of(app)?;
    let now = chrono::Utc::now().timestamp_millis();
    let change = app.state::<HubState>().update_when(
        &dirs,
        channel,
        |list| {
            // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
            ensure_channel(app, channel)?;
            let before = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?.state;
            let thread = list.set_state(thread_id, state, now)?;
            Ok(StateChange { summary: summary(thread), changed: before != state })
        },
        |change| change.changed,
    )?;
    if change.changed {
        emit_update(app, &change.summary);
    }
    Ok(change)
}
