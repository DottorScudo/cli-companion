//! Gli URL della Gmail API e la query di ricerca (puri). Gli id che arrivano dal
//! servizio si controllano prima di finire in un percorso.

use super::form::encode_component;

/// La base di tutte le chiamate (host `keys::API_HOST`).
pub const API_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
/// Messaggi richiesti a `messages.list` per pagina.
pub const LIST_PAGE_SIZE: usize = 100;
/// Lunghezza massima di un id di messaggio.
const ID_MAX: usize = 256;
/// Lunghezza massima di un id di allegato.
const ATTACHMENT_ID_MAX: usize = 2048;

fn id_within(id: &str, max: usize) -> bool {
    !id.is_empty() && id.len() <= max && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Un id di messaggio: `[A-Za-z0-9_-]{1,256}`.
pub fn valid_id(id: &str) -> bool {
    id_within(id, ID_MAX)
}

/// Un id di allegato: stesso alfabeto, fino a 2048 caratteri.
pub fn valid_attachment_id(id: &str) -> bool {
    id_within(id, ATTACHMENT_ID_MAX)
}

/// Un elenco di messaggi che corrispondono a `q`, una pagina alla volta.
pub fn list_url(q: &str, page_token: Option<&str>) -> String {
    let mut url = format!("{API_BASE}/messages?q={}&maxResults={LIST_PAGE_SIZE}", encode_component(q));
    if let Some(token) = page_token {
        url.push_str("&pageToken=");
        url.push_str(&encode_component(token));
    }
    url
}

/// Un messaggio intero, con le parti MIME.
pub fn message_url(id: &str) -> Option<String> {
    valid_id(id).then(|| format!("{API_BASE}/messages/{id}?format=full"))
}

/// Un allegato di un messaggio.
pub fn attachment_url(message_id: &str, attachment_id: &str) -> Option<String> {
    (valid_id(message_id) && valid_attachment_id(attachment_id))
        .then(|| format!("{API_BASE}/messages/{message_id}/attachments/{attachment_id}"))
}

/// Il profilo dell'account (l'indirizzo email).
pub fn profile_url() -> String {
    format!("{API_BASE}/profile")
}

/// `(<ricerca>) after:<secondi>`: la ricerca dell'utente, limitata ai messaggi dal momento indicato.
pub fn search_query(user_query: &str, after_secs: i64) -> String {
    format!("({}) after:{after_secs}", user_query.trim())
}

#[cfg(test)]
mod tests;
