//! Il parsing tollerante di `getUpdates` (puro). Il JSON arriva dalla rete: ogni
//! campo mancante o strano rende ignorato l'aggiornamento, mai un panico e mai un
//! errore dell'intero giro. Un aggiornamento senza `update_id` valido non conta
//! nemmeno per il cursore.

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

const CHAT_PRIVATE: &str = "private";
/// Gli stati con cui un bot entra in una chat.
const JOINED_STATUSES: [&str; 2] = ["member", "administrator"];

/// Un messaggio di una chat privata, come lo legge il connettore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub chat_id: i64,
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
    /// Il bot e' stato aggiunto a una chat non privata: se ne esce.
    JoinedChat(i64),
    /// Ignorato; il motivo (o il tipo del contenuto) e' per il log.
    Ignored(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub id: i64,
    pub kind: UpdateKind,
}

fn non_empty(value: Option<&Value>) -> Option<String> {
    value?.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string)
}

fn chat_of(message: &Value) -> Option<(i64, bool)> {
    let chat = message.get("chat")?;
    let private = chat.get("type")?.as_str()? == CHAT_PRIVATE;
    Some((chat.get("id")?.as_i64()?, private))
}

fn unsupported_kind(message: &Value) -> &'static str {
    UNSUPPORTED_CONTENT.iter().copied().find(|kind| message.get(*kind).is_some()).unwrap_or("other")
}

fn parse_message(message: &Value) -> UpdateKind {
    let Some((chat_id, private)) = chat_of(message) else { return UpdateKind::Ignored("malformed") };
    if !private {
        return UpdateKind::Ignored("non_private_chat");
    }
    let Some(from) = message.get("from") else { return UpdateKind::Ignored("sender_missing") };
    if from.get("is_bot").and_then(Value::as_bool) == Some(true) {
        return UpdateKind::Ignored("bot_sender");
    }
    let (Some(sender_id), Some(message_id)) =
        (from.get("id").and_then(Value::as_i64), message.get("message_id").and_then(Value::as_i64))
    else {
        return UpdateKind::Ignored("malformed");
    };
    let parsed = ChatMessage {
        chat_id,
        message_id,
        sender_id,
        sender_name: sender_name(from),
        text: non_empty(message.get("text")),
        caption: non_empty(message.get("caption")),
        reply_to: message.get("reply_to_message").and_then(|r| r.get("message_id")?.as_i64()),
        media_group: non_empty(message.get("media_group_id")),
        files: message_files(message, message_id),
    };
    if parsed.text.is_none() && parsed.caption.is_none() && parsed.files.is_empty() {
        return UpdateKind::Ignored(unsupported_kind(message));
    }
    UpdateKind::Message(parsed)
}

fn parse_member_change(change: &Value) -> UpdateKind {
    let status = change.get("new_chat_member").and_then(|m| m.get("status")?.as_str());
    match chat_of(change) {
        Some((chat_id, false)) if status.is_some_and(|s| JOINED_STATUSES.contains(&s)) => {
            UpdateKind::JoinedChat(chat_id)
        }
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
