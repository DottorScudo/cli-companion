//! Aggiunta di un messaggio di testo a un thread, con salvataggio ed evento
//! `hub-update`: la usano `hub_send` (i messaggi dell'utente, anche con gli
//! allegati) e il motore dei segretari (risposte e avvisi). La risposta di un
//! segretario offre anche una notifica di sistema (`notify`). Bloccante: va
//! chiamata fuori dal main thread.

use super::attachments::{self, Claimed};
use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, hub_dirs_of};
use super::model::{Attachment, Author, Message};
use super::notify::{HubNotifier, NoticeKind};
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
    append(app, channel, thread_id, text, Vec::new())
}

/// Messaggio dell'utente con gli allegati `attachment_ids` (gia' nello
/// staging, vedi `attachments`): li sposta nella cartella del thread, salva il
/// messaggio e, se il salvataggio fallisce, li riporta nello staging. Senza
/// allegati e' `append_text`.
pub fn append_user_message(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    text: TextMessage,
    attachment_ids: &[String],
) -> Result<Message, String> {
    attachments::validate_ids(attachment_ids)?;
    if attachment_ids.is_empty() {
        return append_text(app, channel, thread_id, text);
    }
    let dirs = hub_dirs_of(app)?;
    // Niente cartelle per un canale o un thread che non ci sono.
    ensure_channel(app, channel)?;
    if !app.state::<HubState>().read(&dirs, channel, |list| list.thread(thread_id).is_some()) {
        return Err(THREAD_NOT_FOUND.to_string());
    }
    let claimed = Claimed::take(&dirs.attachments, channel, thread_id, attachment_ids)?;
    append(app, channel, thread_id, text, claimed.attachments().to_vec()).inspect_err(|_| claimed.release())
}

fn append(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    text: TextMessage,
    attachments: Vec<Attachment>,
) -> Result<Message, String> {
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    // Notifica solo la risposta di un segretario: mai l'utente ne' il motore.
    let replying_bot = match &text.author {
        Author::Bot { name } => Some(name.clone()),
        Author::User | Author::System => None,
    };
    let new = NewMessage {
        id: state.next_id(MESSAGE_ID_PREFIX, now),
        author: text.author,
        body: text.body,
        reply_to: text.reply_to,
        event: None,
    };
    let (message, touched) = state.update(&dirs, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
        ensure_channel(app, channel)?;
        let message = list.add_message_with_attachments(thread_id, new, attachments, now)?.clone();
        let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
        Ok((message, summary(thread)))
    })?;
    emit_update(app, &touched);
    if let Some(bot) = replying_bot {
        app.state::<HubNotifier>().notify(app, &touched, NoticeKind::Reply { bot }, &message.body);
    }
    Ok(message)
}
