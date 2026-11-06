//! Eliminare un thread a mano (`hub_delete_thread`), in qualunque stato e in
//! qualunque canale. Si toglie solo il thread: i task collegati restano sulla
//! board (con worktree e terminali) e un loro evento successivo apre un thread
//! nuovo. Con il thread se ne vanno la cartella dei suoi allegati (cancellata da
//! `HubState::update` dopo il salvataggio), le sessioni e i lavori dei
//! segretari e la notifica in attesa; poi gli ascoltatori del ciclo di vita (le
//! sotto-chat dei canali di contatto) e l'evento `hub-thread-removed`.

use super::channel::Channel;
use super::commands::{ensure_channel, ensure_enabled, hub_dirs_of};
use super::engine::SecretaryEngine;
use super::lifecycle::HubLifecycleListeners;
use super::notify::{HubNotifier, HubThreadRef};
use super::state::HubState;
use tauri::{AppHandle, Emitter, Manager};

/// Evento verso il frontend dopo l'eliminazione di un thread (payload: `HubThreadRef`).
pub const HUB_THREAD_REMOVED_EVENT: &str = "hub-thread-removed";

#[tauri::command]
pub async fn hub_delete_thread(app: AppHandle, channel: String, thread_id: String) -> Result<(), String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    crate::blocking(move || delete(&app, &channel, &thread_id)).await
}

/// Toglie il thread dal file del canale e, solo dopo il salvataggio riuscito,
/// ferma cio' che lavorava su di esso ed emette l'evento. Bloccante: va
/// chiamata fuori dal main thread.
fn delete(app: &AppHandle, channel: &Channel, thread_id: &str) -> Result<(), String> {
    let dirs = hub_dirs_of(app)?;
    app.state::<HubState>().update(&dirs, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
        ensure_channel(app, channel)?;
        list.remove_thread(thread_id).map(drop)
    })?;
    app.state::<SecretaryEngine>().forget_thread(app, channel, thread_id);
    app.state::<HubNotifier>().discard_thread(channel, thread_id);
    // Fuori da ogni lock dell'HUB: gli ascoltatori sono rapidi, la rete la fanno i loro task.
    app.state::<HubLifecycleListeners>().thread_removed(app, channel, thread_id);
    emit_removed(app, channel, thread_id);
    Ok(())
}

fn emit_removed(app: &AppHandle, channel: &Channel, thread_id: &str) {
    let payload = HubThreadRef { channel: channel.as_str().to_string(), thread_id: thread_id.to_string() };
    if let Err(e) = app.emit(HUB_THREAD_REMOVED_EVENT, payload) {
        log::error!("hub: evento {HUB_THREAD_REMOVED_EVENT} non inviato: {e}");
    }
}
