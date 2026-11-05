//! Gli id esterni dei messaggi, `<chat>:<messaggio>` (puri): sono la base della
//! dedupe del runtime e del legame fra una risposta e il messaggio a cui risponde.

/// L'id esterno di un messaggio di una chat.
pub fn external_id(chat_id: i64, message_id: i64) -> String {
    format!("{chat_id}:{message_id}")
}

/// L'id del messaggio di `reply_to_external` se e' della stessa `chat`.
pub fn reply_message_id(chat: &str, reply_to_external: Option<&str>) -> Option<i64> {
    let (reply_chat, message) = reply_to_external?.split_once(':')?;
    if reply_chat != chat {
        return None;
    }
    message.parse().ok()
}

#[cfg(test)]
mod tests;
