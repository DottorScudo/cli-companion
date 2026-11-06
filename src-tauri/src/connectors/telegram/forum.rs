//! Il supergruppo forum (puro): quali chat sono gruppi, quali hanno i topic, il nome
//! di un topic e l'id di un topic come sotto-chat del runtime.

use crate::connectors::text::single_line;

/// Gli id dei supergruppi (e quindi dei forum) iniziano cosi'.
const SUPERGROUP_PREFIX: &str = "-100";
/// Il limite di Telegram per il nome di un topic, in unita' UTF-16 (sta anche nei caratteri).
pub const TOPIC_NAME_UNITS_MAX: usize = 128;
/// Il nome di un topic con un titolo vuoto (Telegram rifiuta un nome vuoto).
const TOPIC_NAME_FALLBACK: &str = "Thread";

/// La chat e' un gruppo (gli id dei gruppi sono negativi, quelli degli utenti positivi)?
pub fn is_group(chat: &str) -> bool {
    chat.starts_with('-')
}

/// La chat e' un supergruppo, l'unico tipo di gruppo che puo' avere i topic? Il
/// connettore non accetta altri gruppi (vedi `updates`), quindi un supergruppo
/// collegato e' un forum.
pub fn is_supergroup(chat: &str) -> bool {
    chat.strip_prefix(SUPERGROUP_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        && chat.parse::<i64>().is_ok()
}

/// Il nome di un topic: il titolo su una sola riga, al piu' `TOPIC_NAME_UNITS_MAX`
/// unita' UTF-16, mai tagliato dentro un carattere; un titolo vuoto ha un nome fisso.
pub fn topic_name(title: &str) -> String {
    let line = single_line(title, TOPIC_NAME_UNITS_MAX);
    let mut used = 0usize;
    let cut: String = line
        .chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= TOPIC_NAME_UNITS_MAX
        })
        .collect();
    let name = cut.trim_end();
    if name.is_empty() {
        TOPIC_NAME_FALLBACK.to_string()
    } else {
        name.to_string()
    }
}

/// La sotto-chat del runtime per un topic.
pub fn subchat_of(topic_id: i64) -> String {
    topic_id.to_string()
}

/// L'id del topic di una sotto-chat: un intero positivo, altrimenti `None`.
pub fn topic_id(subchat: &str) -> Option<i64> {
    subchat.parse::<i64>().ok().filter(|id| *id > 0)
}

#[cfg(test)]
mod tests;
