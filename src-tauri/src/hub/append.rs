//! Aggiunta di un messaggio di testo a un thread, con salvataggio ed evento
//! `hub-update`: la usano `hub_send` (i messaggi dell'utente, anche con gli
//! allegati) e il motore dei segretari (risposte e avvisi). La risposta di un
//! segretario offre anche una notifica di sistema (`notify`) e un messaggio che
//! l'utente scrive nell'app lo sanno i canali di contatto (`app_message_written`,
//! mai uno arrivato da un canale di contatto); un testo lungo
//! diviso in parti (`text::parts`) si scrive tutto insieme con `append_parts`.
//! Un messaggio con un'origine gia' presente nel thread (lo stesso messaggio di
//! un canale di contatto, ripresentato dopo un riavvio) non si scrive di nuovo.
//! Bloccante: va chiamata fuori dal main thread.

use super::attachments::{self, Claimed};
use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, hub_dirs_of};
use super::model::{Attachment, Author, Message, MessageOrigin};
use super::notify::{HubNotifier, NoticeKind};
use super::state::{HubState, MESSAGE_ID_PREFIX};
use super::threads::{message_from, summary, NewMessage, THREAD_NOT_FOUND};
use tauri::{AppHandle, Manager};

/// Il testo da aggiungere: `reply_to` deve essere un messaggio dello stesso thread.
pub struct TextMessage {
    pub author: Author,
    pub body: String,
    pub reply_to: Option<String>,
}

/// Com'e' andata l'aggiunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Appended {
    /// Il messaggio scritto ora.
    Written(Message),
    /// Il thread aveva gia' un messaggio con la stessa origine: nulla e' stato
    /// scritto (ne' emesso), ecco quello.
    Duplicate(Message),
}

impl Appended {
    pub fn into_message(self) -> Message {
        match self {
            Self::Written(message) | Self::Duplicate(message) => message,
        }
    }
}

/// Aggiunge il messaggio (validato da `ChannelThreads::add_message`), lo salva
/// ed emette `hub-update`. Il canale di un workspace deve esistere ancora.
pub fn append_text(app: &AppHandle, channel: &Channel, thread_id: &str, text: TextMessage) -> Result<Message, String> {
    append(app, channel, thread_id, text, Vec::new(), None).map(Appended::into_message)
}

/// Le parti di un testo lungo (`text::parts::split`) come messaggi consecutivi
/// di `author`, scritti insieme sotto lo stesso lock: nessun altro messaggio in
/// mezzo, nulla di scritto se una parte e' rifiutata, un solo `hub-update` e,
/// per la risposta di un segretario, una sola notifica (sulla prima parte).
pub fn append_parts(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    author: Author,
    parts: Vec<String>,
) -> Result<Vec<Message>, String> {
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let bot = replying_bot(&author);
    let news: Vec<NewMessage> = parts
        .into_iter()
        .map(|body| NewMessage {
            id: state.next_id(MESSAGE_ID_PREFIX, now),
            author: author.clone(),
            body,
            reply_to: None,
            event: None,
            origin: None,
        })
        .collect();
    let (messages, touched) = state.update(&dirs, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
        ensure_channel(app, channel)?;
        let messages = news
            .into_iter()
            .map(|new| list.add_message(thread_id, new, now).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
        Ok((messages, summary(thread)))
    })?;
    emit_update(app, &touched);
    if let (Some(bot), Some(first)) = (bot, messages.first()) {
        app.state::<HubNotifier>().notify(app, &touched, &first.id, NoticeKind::Reply { bot }, &first.body);
    }
    Ok(messages)
}

/// Il segretario che risponde, se l'autore e' un segretario: solo la sua
/// risposta notifica, mai l'utente (anche se scrive da un canale di contatto)
/// ne' il motore.
fn replying_bot(author: &Author) -> Option<String> {
    match author {
        Author::Bot { name } => Some(name.clone()),
        Author::User | Author::System | Author::External { .. } => None,
    }
}

/// Il messaggio e' dell'utente ed e' stato scritto nell'app, non in un canale di
/// contatto: solo questi si rispecchiano nei canali (mai un messaggio al canale da cui
/// viene, per non creare un'eco infinita).
fn is_app_user_message(message: &Message) -> bool {
    message.author == Author::User && message.origin.is_none()
}

/// Messaggio dell'utente con gli allegati `attachment_ids` (gia' nello
/// staging, vedi `attachments`): li sposta nella cartella del thread, salva il
/// messaggio e, se il salvataggio fallisce o il messaggio e' un doppione, li
/// riporta nello staging. `origin` dice da dove viene un messaggio scritto in un
/// canale di contatto (vedi `send`). Senza allegati ne' origine e' `append_text`.
pub fn append_user_message(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    text: TextMessage,
    attachment_ids: &[String],
    origin: Option<MessageOrigin>,
) -> Result<Appended, String> {
    attachments::validate_ids(attachment_ids)?;
    if attachment_ids.is_empty() {
        return append(app, channel, thread_id, text, Vec::new(), origin);
    }
    let dirs = hub_dirs_of(app)?;
    // Niente cartelle per un canale o un thread che non ci sono.
    ensure_channel(app, channel)?;
    if !app.state::<HubState>().read(&dirs, channel, |list| list.thread(thread_id).is_some()) {
        return Err(THREAD_NOT_FOUND.to_string());
    }
    let claimed = Claimed::take(&dirs.attachments, channel, thread_id, attachment_ids)?;
    match append(app, channel, thread_id, text, claimed.attachments().to_vec(), origin) {
        Ok(Appended::Written(message)) => Ok(Appended::Written(message)),
        other => {
            claimed.release();
            other
        }
    }
}

fn append(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    text: TextMessage,
    attachments: Vec<Attachment>,
    origin: Option<MessageOrigin>,
) -> Result<Appended, String> {
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let replying_bot = replying_bot(&text.author);
    let new = NewMessage {
        id: state.next_id(MESSAGE_ID_PREFIX, now),
        author: text.author,
        body: text.body,
        reply_to: text.reply_to,
        event: None,
        origin,
    };
    // Il riassunto del thread toccato; nessuno se il messaggio e' un doppione.
    let (message, touched) = state.update_when(
        &dirs,
        channel,
        |list| {
            // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
            ensure_channel(app, channel)?;
            let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
            if let Some(found) = new.origin.as_ref().and_then(|o| message_from(thread, o)) {
                return Ok((found.clone(), None));
            }
            let message = list.add_message_with_attachments(thread_id, new, attachments, now)?.clone();
            let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
            Ok((message, Some(summary(thread))))
        },
        |(_, touched)| touched.is_some(),
    )?;
    let Some(touched) = touched else { return Ok(Appended::Duplicate(message)) };
    emit_update(app, &touched);
    if let Some(bot) = replying_bot {
        app.state::<HubNotifier>().notify(app, &touched, &message.id, NoticeKind::Reply { bot }, &message.body);
    }
    if is_app_user_message(&message) {
        app.state::<HubNotifier>().app_message_written(app, &touched, &message);
    }
    Ok(Appended::Written(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::model::MessageKind;

    fn message(author: Author, origin: Option<MessageOrigin>) -> Message {
        Message {
            id: "msg-1-1".into(),
            author,
            kind: MessageKind::Text,
            body: "ciao".into(),
            reply_to: None,
            event: None,
            attachments: Vec::new(),
            origin,
            created_at: 0,
        }
    }

    fn origin() -> MessageOrigin {
        MessageOrigin { connector: "telegram".into(), instance: "inst-1".into(), external_id: "42".into() }
    }

    #[test]
    fn only_the_users_own_app_messages_are_mirrored() {
        assert!(is_app_user_message(&message(Author::User, None)));
        assert!(!is_app_user_message(&message(Author::User, Some(origin()))), "viene da un canale di contatto");
        for author in [Author::System, Author::Bot { name: "ada".into() }] {
            assert!(!is_app_user_message(&message(author, None)));
        }
    }
}
