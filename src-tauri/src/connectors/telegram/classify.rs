//! La risposta della Bot API come risultato o come errore del connettore (pura).
//! Telegram risponde `{ok, result}` oppure `{ok: false, error_code, description,
//! parameters: {retry_after}}`. La `description` non si riporta mai: i messaggi
//! sono fissi, al piu' con il codice.
//!
//! Solo gli errori del token fermano il supervisore: 400/403 (per esempio un utente
//! che ha bloccato il bot) e 409 (webhook attivo o un altro client di `getUpdates`)
//! si riprovano.

use crate::connectors::contracts::ConnectorError;
use serde_json::Value;
use std::time::Duration;

const CODE_UNAUTHORIZED: u64 = 401;
const CODE_NOT_FOUND: u64 = 404;
const CODE_TOO_MANY: u64 = 429;

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

/// L'errore per un codice: quello del JSON (`error_code`) se c'e', altrimenti lo stato HTTP.
fn error_for(code: u64, parsed: Option<&Value>) -> ConnectorError {
    match code {
        CODE_UNAUTHORIZED | CODE_NOT_FOUND => ConnectorError::Auth("token del bot rifiutato da Telegram".to_string()),
        CODE_TOO_MANY => ConnectorError::RateLimited { retry_after: retry_after(parsed) },
        _ => ConnectorError::Transient(format!("Telegram ha risposto con errore {code}")),
    }
}

/// L'errore di una risposta non riuscita (mai un `Ok`): per le risposte non JSON, come
/// il download di un file, conta solo lo stato HTTP.
pub fn error_of(status: u16, body: &[u8]) -> ConnectorError {
    let parsed = json_of(body);
    let code = parsed.as_ref().and_then(|p| p.get("error_code")?.as_u64()).unwrap_or_else(|| u64::from(status));
    error_for(code, parsed.as_ref())
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
