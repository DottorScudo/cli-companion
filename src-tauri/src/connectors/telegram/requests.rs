//! I corpi JSON delle chiamate alla Bot API (puri). Testo semplice: mai `parse_mode`.

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

pub fn send_message_body(chat_id: i64, text: &str, reply_to: Option<i64>) -> Value {
    let mut body = json!({
        "chat_id": chat_id,
        "text": text,
        "link_preview_options": { "is_disabled": true },
    });
    if let (Some(message_id), Some(map)) = (reply_to, body.as_object_mut()) {
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

#[cfg(test)]
mod tests;
