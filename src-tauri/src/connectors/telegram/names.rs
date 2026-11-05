//! Nomi visualizzati e username che arrivano da Telegram (puri): ripuliti prima di
//! finire in un DTO, in un messaggio o in un URL.

use crate::connectors::text::clean_display;
use serde_json::Value;

/// Lunghezza massima, in caratteri, del nome di un mittente.
pub const SENDER_NAME_MAX: usize = 60;
/// Lunghezza massima del nome visualizzato del bot.
pub const BOT_NAME_MAX: usize = 64;
/// Lunghezza massima di uno username.
const USERNAME_MAX: usize = 64;

fn text_field<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    object.get(key)?.as_str().filter(|s| !s.trim().is_empty())
}

/// `[A-Za-z0-9_]{1,64}`: sicuro in un URL.
pub fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= USERNAME_MAX
        && username.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Come si presenta il mittente (`from`): nome e cognome, altrimenti `@username`; vuoto se ignoto.
pub fn sender_name(from: &Value) -> String {
    let parts: Vec<&str> = ["first_name", "last_name"].iter().filter_map(|key| text_field(from, key)).collect();
    let full = clean_display(&parts.join(" "), SENDER_NAME_MAX);
    if !full.is_empty() {
        return full;
    }
    match text_field(from, "username").filter(|u| valid_username(u)) {
        Some(username) => clean_display(&format!("@{username}"), SENDER_NAME_MAX),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;
