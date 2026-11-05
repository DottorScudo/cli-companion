//! Validazione semantica di configurazione e filtri (pura). I messaggi sono per l'utente.

use super::ascii::is_visible_ascii;
use super::keys::{CLIENT_ID_KEY, QUERY_KEY};
use serde_json::Value;

/// Lunghezza massima, in caratteri, di una ricerca.
pub const QUERY_MAX: usize = 500;
/// Lunghezza massima di un client id OAuth.
pub const CLIENT_ID_MAX: usize = 256;
/// Il suffisso di ogni client id OAuth di Google.
pub const CLIENT_ID_SUFFIX: &str = ".apps.googleusercontent.com";

/// La ricerca finisce in `(<ricerca>) after:<secondi>`: parentesi e virgolette devono
/// essere bilanciate, altrimenti cambierebbero il senso del resto.
pub fn validate_query(raw: &str) -> Result<(), String> {
    let query = raw.trim();
    let length = query.chars().count();
    if length == 0 {
        return Err("La ricerca non può essere vuota.".to_string());
    }
    if length > QUERY_MAX {
        return Err(format!("La ricerca può avere al massimo {QUERY_MAX} caratteri."));
    }
    if query.chars().any(char::is_control) {
        return Err("La ricerca deve stare su una sola riga, senza caratteri di controllo.".to_string());
    }
    check_balance(query)
}

/// Parentesi bilanciate fuori dalle virgolette e numero pari di virgolette.
fn check_balance(query: &str) -> Result<(), String> {
    let mut depth = 0usize;
    let mut quoted = false;
    for c in query.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth =
                    depth.checked_sub(1).ok_or_else(|| "La ricerca ha una parentesi chiusa di troppo.".to_string())?;
            }
            _ => {}
        }
    }
    if quoted {
        return Err("La ricerca ha una virgoletta non chiusa.".to_string());
    }
    if depth != 0 {
        return Err("La ricerca ha una parentesi non chiusa.".to_string());
    }
    Ok(())
}

/// Il filtro di una regola: `{ "query": "..." }`.
pub fn validate_filter(filter: &Value) -> Result<(), String> {
    match filter.get(QUERY_KEY).and_then(Value::as_str) {
        Some(query) => validate_query(query),
        None => Err("Indica la ricerca da usare.".to_string()),
    }
}

/// Un client id OAuth di Google: ASCII visibile, finisce con `.apps.googleusercontent.com`.
pub fn validate_client_id(raw: &str) -> Result<(), String> {
    let id = raw.trim();
    if !is_visible_ascii(id, CLIENT_ID_MAX) {
        return Err("Il client ID non è valido.".to_string());
    }
    if !id.ends_with(CLIENT_ID_SUFFIX) || id.len() == CLIENT_ID_SUFFIX.len() {
        return Err(format!("Il client ID deve finire con {CLIENT_ID_SUFFIX}."));
    }
    Ok(())
}

/// La configurazione dell'istanza: il client id.
pub fn validate_config(config: &Value) -> Result<(), String> {
    match config.get(CLIENT_ID_KEY).and_then(Value::as_str) {
        Some(client_id) => validate_client_id(client_id),
        None => Err("Indica il client ID OAuth.".to_string()),
    }
}

#[cfg(test)]
mod tests;
