//! Da un messaggio di `messages.get` (`format=full`) a un messaggio letto (puro).

use super::api_urls::valid_id;
use super::html::html_to_text;
use super::mime::{collect, header_value, AttachmentRef, Collected};
use super::words::decode_words;
use crate::connectors::contracts::ConnectorError;
use crate::connectors::text::single_line;
use serde_json::Value;

/// Caratteri di testo tenuti (l'HUB poi taglia a 8000).
pub const TEXT_MAX: usize = 32_000;
/// Lunghezza massima, in caratteri, di oggetto e mittente.
const HEADER_MAX: usize = 1000;
/// Lunghezza massima, in caratteri, dell'intestazione `Date`.
const DATE_MAX: usize = 200;

/// Un messaggio letto da Gmail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub id: String,
    pub thread_id: Option<String>,
    /// `internalDate`: quando Gmail ha ricevuto il messaggio, in millisecondi Unix.
    pub internal_ms: i64,
    pub subject: String,
    /// L'intestazione `From`, ripulita (`Nome <indirizzo>`).
    pub from: String,
    /// L'intestazione `Date`, ripulita; vuota se manca.
    pub date: String,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
}

fn invalid() -> ConnectorError {
    ConnectorError::Transient("messaggio di Gmail in forma inattesa".to_string())
}

/// `internalDate` arriva come stringa di millisecondi.
fn internal_ms_of(v: &Value) -> Option<i64> {
    match v.get("internalDate")? {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

/// Un'intestazione decodificata (RFC 2047), su una riga, tagliata.
fn header_text(payload: &Value, name: &str, max: usize) -> String {
    header_value(payload, name).map(|raw| single_line(&decode_words(raw), max)).unwrap_or_default()
}

/// A capo uniformi e niente caratteri di controllo (a parte a capo e tabulazione), tagliato a `TEXT_MAX`.
fn normalize_text(raw: &str) -> String {
    let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
    let kept: String = unified
        .chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .take(TEXT_MAX.saturating_mul(2))
        .collect();
    kept.trim().chars().take(TEXT_MAX).collect()
}

/// La parte semplice, altrimenti l'HTML convertito, altrimenti l'anteprima (`snippet`).
fn body_text(collected: &Collected, snippet: Option<&str>) -> String {
    let plain = collected.plain.as_deref().filter(|t| !t.trim().is_empty()).map(normalize_text);
    let html = || collected.html.as_deref().map(html_to_text).map(|t| normalize_text(&t)).filter(|t| !t.is_empty());
    let preview = || snippet.map(html_to_text).map(|t| normalize_text(&t));
    plain.or_else(html).or_else(preview).unwrap_or_default()
}

/// Legge un messaggio. Un messaggio senza id o senza `internalDate` e' un errore
/// transitorio: chi legge lo salta, non deve fermare il connettore.
pub fn parse_message(v: &Value) -> Result<ParsedMessage, ConnectorError> {
    let id = v.get("id").and_then(Value::as_str).filter(|id| valid_id(id)).ok_or_else(invalid)?.to_string();
    let internal_ms = internal_ms_of(v).ok_or_else(invalid)?;
    let thread_id = v.get("threadId").and_then(Value::as_str).filter(|t| valid_id(t)).map(str::to_string);
    let payload = v.get("payload").unwrap_or(&Value::Null);
    let collected = collect(payload);
    let text = body_text(&collected, v.get("snippet").and_then(Value::as_str));
    Ok(ParsedMessage {
        id,
        thread_id,
        internal_ms,
        subject: header_text(payload, "Subject", HEADER_MAX),
        from: header_text(payload, "From", HEADER_MAX),
        date: header_text(payload, "Date", DATE_MAX),
        text,
        attachments: collected.attachments,
    })
}

#[cfg(test)]
mod tests;
