//! La risposta di `messages.list` (pura): gli id dei messaggi e il token della pagina successiva.

use super::api_urls::{valid_id, LIST_PAGE_SIZE};
use super::ascii::is_visible_ascii;
use crate::connectors::contracts::ConnectorError;
use serde_json::Value;

/// Id letti da una pagina, al massimo: oltre la dimensione richiesta Gmail non risponde.
pub const IDS_MAX: usize = LIST_PAGE_SIZE;
/// Lunghezza massima del token di una pagina.
const PAGE_TOKEN_MAX: usize = 1024;

/// Una pagina di risultati.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    /// Gli id, nell'ordine di Gmail (dal piu' recente).
    pub ids: Vec<String>,
    pub next_page_token: Option<String>,
}

/// Gli id validi di `messages` (`[{id, threadId}]`); un id illeggibile si salta.
pub fn parse_list(body: &Value) -> Result<ListPage, ConnectorError> {
    if !body.is_object() {
        return Err(ConnectorError::Transient("risposta di Gmail non valida".to_string()));
    }
    let ids = match body.get("messages") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(messages)) => messages
            .iter()
            .filter_map(|m| m.get("id")?.as_str())
            .filter(|id| valid_id(id))
            .take(IDS_MAX)
            .map(str::to_string)
            .collect(),
        Some(_) => return Err(ConnectorError::Transient("risposta di Gmail non valida".to_string())),
    };
    let next_page_token = body
        .get("nextPageToken")
        .and_then(Value::as_str)
        .filter(|t| is_visible_ascii(t, PAGE_TOKEN_MAX))
        .map(str::to_string);
    Ok(ListPage { ids, next_page_token })
}

#[cfg(test)]
mod tests;
