//! I corpi JSON delle chiamate alla Bot API (puri). I messaggi vanno in HTML
//! (`parse_mode: "HTML"`, il testo gia' convertito da `format`), o in testo semplice
//! senza `parse_mode` quando Telegram ha rifiutato l'HTML di quella parte.

use super::forum::topic_name;
use serde_json::{json, Value};

/// Secondi di attesa del long polling lato Telegram (sotto `limits::LONG_POLL_MAX`).
pub const POLL_WAIT_SECS: u64 = 50;
/// Aggiornamenti richiesti per chiamata.
const POLL_LIMIT: u32 = 100;
/// Gli unici tipi di aggiornamento che interessano.
const ALLOWED_UPDATES: [&str; 2] = ["message", "my_chat_member"];

/// Il primo giro parte da "adesso": solo l'ultimo aggiornamento, senza attesa.
pub fn latest_update_body() -> Value {
    json!({ "offset": -1, "timeout": 0 })
}

pub fn poll_body(offset: i64) -> Value {
    json!({
        "offset": offset,
        "timeout": POLL_WAIT_SECS,
        "limit": POLL_LIMIT,
        "allowed_updates": ALLOWED_UPDATES,
    })
}

/// Toglie un webhook eventuale (impedirebbe `getUpdates`) senza perdere i messaggi in attesa.
pub fn delete_webhook_body() -> Value {
    json!({ "drop_pending_updates": false })
}

/// Dove va un messaggio: la chat e, in un forum, il topic (`None` = «General» o chat privata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination {
    pub chat_id: i64,
    pub topic_id: Option<i64>,
}

/// Come Telegram legge il testo di un messaggio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    /// Il sottoinsieme HTML della Bot API.
    Html,
    /// Testo semplice: nessun `parse_mode`.
    Plain,
}

const PARSE_MODE_HTML: &str = "HTML";

pub fn send_message_body(to: Destination, text: &str, mode: TextMode, reply_to: Option<i64>) -> Value {
    let mut body = json!({
        "chat_id": to.chat_id,
        "text": text,
        "link_preview_options": { "is_disabled": true },
    });
    let Some(map) = body.as_object_mut() else { return body };
    if mode == TextMode::Html {
        map.insert("parse_mode".to_string(), json!(PARSE_MODE_HTML));
    }
    if let Some(topic_id) = to.topic_id {
        map.insert("message_thread_id".to_string(), json!(topic_id));
    }
    if let Some(message_id) = reply_to {
        map.insert(
            "reply_parameters".to_string(),
            json!({ "message_id": message_id, "allow_sending_without_reply": true }),
        );
    }
    body
}

pub fn leave_chat_body(chat_id: i64) -> Value {
    json!({ "chat_id": chat_id })
}

pub fn get_file_body(file_id: &str) -> Value {
    json!({ "file_id": file_id })
}

/// `getChatMember`: lo stato di `user_id` (qui il bot stesso) nella chat.
pub fn get_chat_member_body(chat_id: i64, user_id: i64) -> Value {
    json!({ "chat_id": chat_id, "user_id": user_id })
}

/// `createForumTopic`: il nome si adatta al limite di Telegram (`forum::topic_name`).
pub fn create_forum_topic_body(chat_id: i64, title: &str) -> Value {
    json!({ "chat_id": chat_id, "name": topic_name(title) })
}

/// `editForumTopic`: solo il nome, l'icona resta.
pub fn edit_forum_topic_body(chat_id: i64, topic_id: i64, title: &str) -> Value {
    json!({ "chat_id": chat_id, "message_thread_id": topic_id, "name": topic_name(title) })
}

/// `closeForumTopic`, `reopenForumTopic` e `deleteForumTopic`.
pub fn topic_body(chat_id: i64, topic_id: i64) -> Value {
    json!({ "chat_id": chat_id, "message_thread_id": topic_id })
}

#[cfg(test)]
mod tests;
