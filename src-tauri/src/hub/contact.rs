//! L'HUB visto da un canale di contatto (docs/CONNECTORS.MD §10): i canali con
//! il loro nome, i thread aperti di tutti i canali e la creazione di un thread.
//! Tutte bloccanti (scrivono o leggono su disco): vanno chiamate fuori dal main
//! thread, come fa il runtime dei connettori. Gli errori sono testi in italiano.
//! Le parti pure (ordine dei canali, selezione dei thread aperti) sono provate
//! senza `AppHandle`.

use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, ensure_enabled, hub_dirs_of};
use super::model::{Thread, ThreadState, ThreadSummary};
use super::notify::text::channel_name;
use super::state::{HubState, THREAD_ID_PREFIX};
use super::threads::summary;
use crate::workspaces::WorkspaceStore;
use tauri::{AppHandle, Manager};

/// Un canale con il nome che vede l'utente (mai l'id del workspace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEntry {
    pub channel: Channel,
    pub name: String,
}

/// Un thread aperto, con il nome del suo canale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadEntry {
    pub channel: Channel,
    pub channel_name: String,
    pub thread_id: String,
    pub title: String,
    /// Un evento aspetta l'utente (proposta, task fallito o in attenzione).
    pub waiting_user: bool,
    pub updated_at: i64,
}

/// Il Generale per primo, poi i workspace (`(id, nome)`) per nome senza badare alle maiuscole.
fn channel_entries(workspaces: impl IntoIterator<Item = (String, String)>) -> Vec<ChannelEntry> {
    let mut named: Vec<(String, String)> = workspaces.into_iter().collect();
    named.sort_by_cached_key(|(_, name)| name.to_lowercase());
    let general = ChannelEntry { channel: Channel::General, name: channel_name(&Channel::General, None) };
    let others = named.into_iter().map(|(id, name)| {
        let channel = Channel::Workspace(id);
        let shown = channel_name(&channel, Some(&name));
        ChannelEntry { channel, name: shown }
    });
    std::iter::once(general).chain(others).collect()
}

/// I thread aperti di un canale, dai riassunti.
fn open_entries(entry: &ChannelEntry, summaries: Vec<ThreadSummary>) -> Vec<ThreadEntry> {
    summaries
        .into_iter()
        .filter(|s| s.state == ThreadState::Open)
        .map(|s| ThreadEntry {
            channel: entry.channel.clone(),
            channel_name: entry.name.clone(),
            thread_id: s.id,
            title: s.title,
            waiting_user: s.waiting_user,
            updated_at: s.updated_at,
        })
        .collect()
}

/// I canali dell'HUB: Generale e poi un canale per workspace. Errore con l'HUB spento.
pub fn channels(app: &AppHandle) -> Result<Vec<ChannelEntry>, String> {
    ensure_enabled(app)?;
    let workspaces = app.state::<WorkspaceStore>().all();
    Ok(channel_entries(workspaces.into_iter().map(|w| (w.id, w.name))))
}

/// I thread aperti di tutti i canali (l'ordine lo decide chi li mostra).
pub fn open_threads(app: &AppHandle) -> Result<Vec<ThreadEntry>, String> {
    let entries = channels(app)?;
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let mut open = Vec::new();
    for entry in &entries {
        let summaries = state.read(&dirs, &entry.channel, |list| list.summaries());
        open.extend(open_entries(entry, summaries));
    }
    Ok(open)
}

/// Un thread libero aperto dall'utente (comando `hub_create_thread` e `/new` dei canali di contatto).
pub fn create_thread(app: &AppHandle, channel: &Channel, title: &str) -> Result<Thread, String> {
    ensure_enabled(app)?;
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let id = state.next_id(THREAD_ID_PREFIX, now);
    let thread = state.update(&dirs, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta
        // e poi cancella anche questo file.
        ensure_channel(app, channel)?;
        list.create_thread(channel, id, title, now).cloned()
    })?;
    emit_update(app, &summary(&thread));
    Ok(thread)
}

#[cfg(test)]
mod tests;
