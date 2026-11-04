//! Aggiunta di un messaggio di testo a un thread, con salvataggio ed evento
//! `hub-update`: la usano `hub_send` (i messaggi dell'utente) e il motore dei
//! segretari (risposte e avvisi). Bloccante: va chiamata fuori dal main thread.

use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, threads_dir_of};
use super::model::{Author, Message};
use super::state::{HubState, MESSAGE_ID_PREFIX};
use super::threads::{summary, NewMessage, THREAD_NOT_FOUND};
use tauri::{AppHandle, Manager};

/// Il testo da aggiungere: `reply_to` deve essere un messaggio dello stesso thread.
pub struct TextMessage {
    pub author: Author,
    pub body: String,
    pub reply_to: Option<String>,
}

/// Aggiunge il messaggio (validato da `ChannelThreads::add_message`), lo salva
/// ed emette `hub-update`. Il canale di un workspace deve esistere ancora.
pub fn append_text(app: &AppHandle, channel: &Channel, thread_id: &str, text: TextMessage) -> Result<Message, String> {
    let dir = threads_dir_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let new = NewMessage {
        id: state.next_id(MESSAGE_ID_PREFIX, now),
        author: text.author,
        body: text.body,
        reply_to: text.reply_to,
        event: None,
    };
    let (message, touched) = state.update(&dir, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
        ensure_channel(app, channel)?;
        let message = list.add_message(thread_id, new, now)?.clone();
        let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
        Ok((message, summary(thread)))
    })?;
    emit_update(app, &touched);
    Ok(message)
}
