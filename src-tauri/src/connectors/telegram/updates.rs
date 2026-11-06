//! Il parsing tollerante di `getUpdates` (puro). Il JSON arriva dalla rete: ogni
//! campo mancante o strano rende ignorato l'aggiornamento, mai un panico e mai un
//! errore dell'intero giro. Un aggiornamento senza `update_id` valido non conta
//! nemmeno per il cursore. Si leggono le chat private e i supergruppi forum (con i
//! topic); gli altri gruppi e i canali si ignorano.

use super::files::message_files;
use super::names::sender_name;
use crate::connectors::contracts::RemoteFile;
use serde_json::Value;

/// Contenuti che non si trasportano: il loro tipo finisce nel log, mai il contenuto.
const UNSUPPORTED_CONTENT: [&str; 12] = [
    "sticker",
    "voice",
    "video",
    "video_note",
    "audio",
    "animation",
    "location",
    "contact",
    "poll",
    "venue",
    "dice",
    "game",
];

/// I messaggi di servizio dei topic di un forum: non sono dell'utente.
const TOPIC_SERVICE: [&str; 6] = [
    "forum_topic_created",
    "forum_topic_edited",
    "forum_topic_closed",
    "forum_topic_reopened",
    "general_forum_topic_hidden",
    "general_forum_topic_unhidden",
];

const CHAT_PRIVATE: &str = "private";
const CHAT_SUPERGROUP: &str = "supergroup";
/// Gli stati con cui un bot entra in una chat.
const JOINED_STATUSES: [&str; 2] = ["member", "administrator"];

/// Un messaggio di una chat privata o di un forum, come lo legge il connettore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub chat_id: i64,
    /// Il topic del forum in cui e' scritto; `None` in una chat privata e nel topic «General».
    pub topic_id: Option<i64>,
    pub message_id: i64,
    pub sender_id: i64,
    pub sender_name: String,
    pub text: Option<String>,
    pub caption: Option<String>,
    pub reply_to: Option<i64>,
    pub media_group: Option<String>,
    pub files: Vec<RemoteFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateKind {
    Message(ChatMessage),
    /// Il bot e' stato aggiunto a un gruppo senza i topic (o a un canale): se ne esce.
    JoinedChat(i64),
    /// Ignorato; il motivo (o il tipo del contenuto) e' per il log.
    Ignored(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub id: i64,
    pub kind: UpdateKind,
}

/// Il tipo di una chat, per quello che interessa al connettore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatKind {
    Private,
    /// Un supergruppo con i topic attivi.
    Forum,
    /// Un gruppo senza topic, un canale o un tipo sconosciuto.
    Other,
}

fn non_empty(value: Option<&Value>) -> Option<String> {
    value?.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string)
}

fn flag(object: &Value, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool) == Some(true)
}

fn chat_of(object: &Value) -> Option<(i64, ChatKind)> {
    let chat = object.get("chat")?;
    let kind = match chat.get("type")?.as_str()? {
        CHAT_PRIVATE => ChatKind::Private,
        CHAT_SUPERGROUP if flag(chat, "is_forum") => ChatKind::Forum,
        _ => ChatKind::Other,
    };
    Some((chat.get("id")?.as_i64()?, kind))
}

fn unsupported_kind(message: &Value) -> &'static str {
    UNSUPPORTED_CONTENT.iter().copied().find(|kind| message.get(*kind).is_some()).unwrap_or("other")
}

/// Il topic di un messaggio di un forum: solo se e' davvero un messaggio di un topic.
fn topic_of(message: &Value, kind: ChatKind) -> Option<i64> {
    if kind != ChatKind::Forum || !flag(message, "is_topic_message") {
        return None;
    }
    message.get("message_thread_id")?.as_i64().filter(|id| *id > 0)
}

/// Il messaggio a cui risponde. In un topic ogni messaggio «risponde» al messaggio di
/// servizio che ha aperto il topic (con l'id del topic): quella non e' una risposta.
fn reply_of(message: &Value, topic_id: Option<i64>) -> Option<i64> {
    let replied = message.get("reply_to_message")?.get("message_id")?.as_i64()?;
    (Some(replied) != topic_id).then_some(replied)
}

fn parse_message(message: &Value) -> UpdateKind {
    let Some((chat_id, kind)) = chat_of(message) else { return UpdateKind::Ignored("malformed") };
    if kind == ChatKind::Other {
        return UpdateKind::Ignored("unsupported_chat");
    }
    if TOPIC_SERVICE.iter().any(|key| message.get(*key).is_some()) {
        return UpdateKind::Ignored("topic_service");
    }
    let Some(from) = message.get("from") else { return UpdateKind::Ignored("sender_missing") };
    if flag(from, "is_bot") {
        return UpdateKind::Ignored("bot_sender");
    }
    let (Some(sender_id), Some(message_id)) =
        (from.get("id").and_then(Value::as_i64), message.get("message_id").and_then(Value::as_i64))
    else {
        return UpdateKind::Ignored("malformed");
    };
    let topic_id = topic_of(message, kind);
    let parsed = ChatMessage {
        chat_id,
        topic_id,
        message_id,
        sender_id,
        sender_name: sender_name(from),
        text: non_empty(message.get("text")),
        caption: non_empty(message.get("caption")),
        reply_to: reply_of(message, topic_id),
        media_group: non_empty(message.get("media_group_id")),
        files: message_files(message, message_id),
    };
    if parsed.text.is_none() && parsed.caption.is_none() && parsed.files.is_empty() {
        return UpdateKind::Ignored(unsupported_kind(message));
    }
    UpdateKind::Message(parsed)
}

/// Il bot entra in una chat: in un forum resta, in attesa dell'abbinamento; da un
/// gruppo senza topic o da un canale esce.
fn parse_member_change(change: &Value) -> UpdateKind {
    let joined = change
        .get("new_chat_member")
        .and_then(|m| m.get("status")?.as_str())
        .is_some_and(|s| JOINED_STATUSES.contains(&s));
    match chat_of(change) {
        Some((_, ChatKind::Forum)) if joined => UpdateKind::Ignored("joined_forum"),
        Some((chat_id, ChatKind::Other)) if joined => UpdateKind::JoinedChat(chat_id),
        _ => UpdateKind::Ignored("chat_member"),
    }
}

fn parse_update(raw: &Value) -> Option<Update> {
    let id = raw.get("update_id")?.as_i64()?;
    let kind = if let Some(message) = raw.get("message") {
        parse_message(message)
    } else if let Some(change) = raw.get("my_chat_member") {
        parse_member_change(change)
    } else {
        UpdateKind::Ignored("unsupported_update")
    };
    Some(Update { id, kind })
}

/// Gli aggiornamenti del `result` di `getUpdates`, nell'ordine di arrivo.
pub fn parse_updates(result: &Value) -> Vec<Update> {
    result.as_array().map(|items| items.iter().filter_map(parse_update).collect()).unwrap_or_default()
}

/// Il prossimo offset da chiedere: l'ultimo `update_id` visto piu' uno.
pub fn next_offset(updates: &[Update]) -> Option<i64> {
    updates.iter().map(|u| u.id).max().map(|id| id.saturating_add(1))
}

#[cfg(test)]
mod tests;
