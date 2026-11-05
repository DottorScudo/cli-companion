//! Messaggi di Gmail d'esempio per i test (JSON come lo restituisce `messages.get`).

use super::message::{parse_message, ParsedMessage};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};

pub const THREAD: &str = "thr1";

/// Un messaggio di solo testo.
pub fn message_json(id: &str, internal_ms: i64, subject: &str, text: &str) -> Value {
    json!({
        "id": id,
        "threadId": THREAD,
        "internalDate": internal_ms.to_string(),
        "snippet": "anteprima",
        "payload": {
            "mimeType": "text/plain",
            "headers": [
                {"name": "Subject", "value": subject},
                {"name": "From", "value": "Mario Rossi <mario@gmail.com>"},
                {"name": "Date", "value": "Tue, 14 Nov 2023 22:13:20 +0000"}
            ],
            "body": {"size": text.len(), "data": URL_SAFE_NO_PAD.encode(text.as_bytes())}
        }
    })
}

/// Lo stesso messaggio, gia' letto.
pub fn parsed(id: &str, internal_ms: i64) -> ParsedMessage {
    parse_message(&message_json(id, internal_ms, "Riunione", "Ciao\nmondo")).unwrap()
}
