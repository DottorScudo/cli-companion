//! La risposta HTTP della Gmail API come errore del connettore (pura). Il corpo serve
//! solo a distinguere le cause di un 403 e non si riporta mai: i messaggi sono fissi.
//!
//! `Retry-After` si legge solo come numero di secondi: la forma a data richiede
//! l'orologio (`runtime::backoff::parse_retry_after`) e Google usa i secondi.

use crate::connectors::contracts::ConnectorError;
use serde_json::Value;
use std::time::Duration;

const STATUS_BAD_REQUEST: u16 = 400;
const STATUS_UNAUTHORIZED: u16 = 401;
const STATUS_FORBIDDEN: u16 = 403;
const STATUS_NOT_FOUND: u16 = 404;
const STATUS_TOO_MANY: u16 = 429;
/// Lunghezza massima di un `Retry-After` creduto.
const RETRY_AFTER_MAX_LEN: usize = 16;

/// Motivi di un 403 che sono un limite di frequenza.
const RATE_REASONS: [&str; 3] = ["rateLimitExceeded", "userRateLimitExceeded", "RATE_LIMIT_EXCEEDED"];
/// Motivi di un 403 che dicono che la Gmail API non e' abilitata nel progetto.
const DISABLED_REASONS: [&str; 2] = ["accessNotConfigured", "SERVICE_DISABLED"];

/// Perche' una chiamata non e' riuscita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiFailure {
    /// 401: il token di accesso non vale (scaduto o revocato): si rinnova e si riprova una volta.
    Unauthorized,
    /// 404: l'elemento non esiste piu' (un messaggio cancellato).
    NotFound,
    /// 400: la richiesta non e' valida (per esempio una ricerca che Gmail rifiuta).
    BadRequest,
    /// Tutto il resto, gia' classificato per il supervisore.
    Error(ConnectorError),
}

fn retry_after_of(raw: Option<&str>) -> Option<Duration> {
    let value = raw?.trim();
    if value.is_empty() || value.len() > RETRY_AFTER_MAX_LEN || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok().map(Duration::from_secs)
}

/// Le stringhe che spiegano un 403: `error.errors[].reason`, `error.status`, `error.details[].reason`.
fn forbidden_reasons(body: &[u8]) -> Vec<String> {
    let Ok(parsed) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let Some(error) = parsed.get("error") else {
        return Vec::new();
    };
    let list_reasons = |key: &str| -> Vec<String> {
        error
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|i| i.get("reason")?.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    let mut reasons = list_reasons("errors");
    reasons.extend(list_reasons("details"));
    reasons.extend(error.get("status").and_then(Value::as_str).map(str::to_string));
    reasons
}

fn forbidden(retry_after: Option<&str>, body: &[u8]) -> ConnectorError {
    let reasons = forbidden_reasons(body);
    let has = |set: &[&str]| reasons.iter().any(|r| set.contains(&r.as_str()));
    if has(&RATE_REASONS) {
        ConnectorError::RateLimited { retry_after: retry_after_of(retry_after) }
    } else if has(&DISABLED_REASONS) {
        ConnectorError::Config("Gmail API non abilitata nel progetto Google Cloud".to_string())
    } else {
        ConnectorError::Auth("accesso a Gmail rifiutato".to_string())
    }
}

/// `Ok` per un 2xx, altrimenti il motivo.
pub fn classify(status: u16, retry_after: Option<&str>, body: &[u8]) -> Result<(), ApiFailure> {
    match status {
        200..=299 => Ok(()),
        STATUS_UNAUTHORIZED => Err(ApiFailure::Unauthorized),
        STATUS_NOT_FOUND => Err(ApiFailure::NotFound),
        STATUS_BAD_REQUEST => Err(ApiFailure::BadRequest),
        STATUS_TOO_MANY => {
            Err(ApiFailure::Error(ConnectorError::RateLimited { retry_after: retry_after_of(retry_after) }))
        }
        STATUS_FORBIDDEN => Err(ApiFailure::Error(forbidden(retry_after, body))),
        _ => Err(ApiFailure::Error(ConnectorError::Transient("Gmail non ha risposto come previsto".to_string()))),
    }
}

#[cfg(test)]
mod tests;
