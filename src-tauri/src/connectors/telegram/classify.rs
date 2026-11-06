//! La risposta della Bot API come risultato o come errore del connettore (pura).
//! Telegram risponde `{ok, result}` oppure `{ok: false, error_code, description,
//! parameters: {retry_after}}`. La `description` non si riporta mai: i messaggi
//! sono fissi, al piu' con il codice.
//!
//! Solo gli errori del token fermano il supervisore: 400/403 (per esempio un utente
//! che ha bloccato il bot) e 409 (webhook attivo o un altro client di `getUpdates`)
//! si riprovano. Un 400 per un topic eliminato e' `SubchatGone`: il runtime riapre il topic.
//! Un 400 per l'HTML di un messaggio (`entities_rejected`) lo fa rimandare senza formattazione.

use crate::connectors::contracts::ConnectorError;
use serde_json::Value;
use std::time::Duration;

const CODE_BAD_REQUEST: u64 = 400;
const CODE_UNAUTHORIZED: u64 = 401;
const CODE_NOT_FOUND: u64 = 404;
const CODE_TOO_MANY: u64 = 429;
/// Come Telegram descrive un topic che non c'e' piu' (in minuscolo): si confronta, mai si riporta.
const TOPIC_GONE_MARKERS: [&str; 2] = ["message thread not found", "topic_id_invalid"];
/// Come Telegram descrive un HTML che non riesce a leggere (in minuscolo).
const ENTITIES_REJECTED_MARKERS: [&str; 1] = ["can't parse entities"];

fn json_of(body: &[u8]) -> Option<Value> {
    serde_json::from_slice(body).ok()
}

/// Il `result` di una risposta riuscita.
fn result_of(parsed: &Value) -> Option<&Value> {
    if parsed.get("ok").and_then(Value::as_bool) == Some(true) {
        parsed.get("result")
    } else {
        None
    }
}

/// L'attesa chiesta da Telegram con un 429.
fn retry_after(parsed: Option<&Value>) -> Option<Duration> {
    let seconds = parsed?.get("parameters")?.get("retry_after")?.as_u64()?;
    Some(Duration::from_secs(seconds))
}

/// Un 400 per un topic eliminato (la sotto-chat e' sparita)?
fn topic_gone(parsed: Option<&Value>) -> bool {
    description_contains(parsed, &TOPIC_GONE_MARKERS)
}

/// La `description` (in minuscolo) contiene uno dei `markers`? Si confronta, mai si riporta.
fn description_contains(parsed: Option<&Value>, markers: &[&str]) -> bool {
    let Some(description) = parsed.and_then(|p| p.get("description")?.as_str()) else { return false };
    let description = description.to_ascii_lowercase();
    markers.iter().any(|marker| description.contains(marker))
}

/// Il codice d'errore: quello del JSON (`error_code`) se c'e', altrimenti lo stato HTTP.
fn code_of(status: u16, parsed: Option<&Value>) -> u64 {
    parsed.and_then(|p| p.get("error_code")?.as_u64()).unwrap_or_else(|| u64::from(status))
}

/// L'errore per un codice.
fn error_for(code: u64, parsed: Option<&Value>) -> ConnectorError {
    match code {
        CODE_UNAUTHORIZED | CODE_NOT_FOUND => ConnectorError::Auth("token del bot rifiutato da Telegram".to_string()),
        CODE_TOO_MANY => ConnectorError::RateLimited { retry_after: retry_after(parsed) },
        CODE_BAD_REQUEST if topic_gone(parsed) => ConnectorError::SubchatGone,
        _ => ConnectorError::Transient(format!("Telegram ha risposto con errore {code}")),
    }
}

/// L'errore di una risposta non riuscita (mai un `Ok`): per le risposte non JSON, come
/// il download di un file, conta solo lo stato HTTP.
pub fn error_of(status: u16, body: &[u8]) -> ConnectorError {
    let parsed = json_of(body);
    error_for(code_of(status, parsed.as_ref()), parsed.as_ref())
}

/// Un 400 perche' Telegram non legge l'HTML del testo («can't parse entities»)? Il
/// chiamante rimanda lo stesso testo senza `parse_mode`.
pub fn entities_rejected(status: u16, body: &[u8]) -> bool {
    let parsed = json_of(body);
    code_of(status, parsed.as_ref()) == CODE_BAD_REQUEST
        && description_contains(parsed.as_ref(), &ENTITIES_REJECTED_MARKERS)
}

/// Il `result` di una risposta della Bot API, o l'errore classificato.
pub fn classify(status: u16, body: &[u8]) -> Result<Value, ConnectorError> {
    let parsed = json_of(body);
    match parsed.as_ref().and_then(result_of) {
        Some(result) if (200..300).contains(&status) => Ok(result.clone()),
        // Anche uno 200 che non e' `{ok: true, result}` e' un errore (di forma inattesa).
        _ => Err(error_of(status, body)),
    }
}

#[cfg(test)]
mod tests;
