//! La richiesta HTTP con cui il browser torna sul loopback dopo il consenso (pura) e
//! le risposte fisse. Non si riporta mai in pagina cio' che arriva nella richiesta.

use super::ascii::is_visible_ascii;
use super::form::decode_component;

/// Dimensione massima della testata della richiesta: oltre, la connessione si chiude.
pub const HEAD_MAX: usize = 8 * 1024;
/// Lunghezza massima di un codice di autorizzazione.
const CODE_MAX: usize = 2048;
/// Lunghezza massima dello `state`.
const STATE_MAX: usize = 256;
/// La fine della testata HTTP.
const HEAD_TERMINATOR: &[u8] = b"\r\n\r\n";

/// Cosa ha chiesto il browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    /// Il redirect del consenso riuscito.
    Code { code: String, state: String },
    /// L'utente ha negato il consenso (o Google ha riportato un errore).
    Denied { state: Option<String> },
    /// Qualunque altra richiesta valida (`/favicon.ico`, `/` senza parametri): si risponde 404.
    Other,
}

/// Una richiesta che non si capisce o non e' un `GET` ben formato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackError {
    Malformed,
}

/// Dove finisce la testata: l'indice subito dopo `\r\n\r\n`, `None` se non e' ancora arrivata.
pub fn head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(HEAD_TERMINATOR.len()).position(|w| w == HEAD_TERMINATOR).map(|at| at + HEAD_TERMINATOR.len())
}

/// La riga della richiesta: metodo, destinazione e versione.
fn request_target(head: &[u8]) -> Result<&str, CallbackError> {
    let line_end = head.windows(2).position(|w| w == b"\r\n").unwrap_or(head.len());
    let line = head.get(..line_end).and_then(|l| std::str::from_utf8(l).ok()).ok_or(CallbackError::Malformed)?;
    let mut parts = line.split(' ');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("GET"), Some(target), Some(version), None) if version.starts_with("HTTP/") => Ok(target),
        _ => Err(CallbackError::Malformed),
    }
}

/// Un parametro della query, decodificato.
fn param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (decode_component(key).as_deref() == Some(name)).then(|| decode_component(value)).flatten()
    })
}

fn valid_state(state: Option<String>) -> Option<String> {
    state.filter(|s| is_visible_ascii(s, STATE_MAX))
}

/// Legge la richiesta di redirect: solo `GET /?...`, qualunque altro percorso e' `Other`.
pub fn parse_request(head: &[u8]) -> Result<Callback, CallbackError> {
    let target = request_target(head)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/" || query.is_empty() {
        return Ok(Callback::Other);
    }
    let state = valid_state(param(query, "state"));
    if param(query, "error").is_some() {
        return Ok(Callback::Denied { state });
    }
    let code = param(query, "code").filter(|c| is_visible_ascii(c, CODE_MAX));
    match (code, state) {
        (Some(code), Some(state)) => Ok(Callback::Code { code, state }),
        _ => Err(CallbackError::Malformed),
    }
}

/// Le pagine con cui si risponde al browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Success,
    Failure,
    NotFound,
}

impl Page {
    fn status(self) -> &'static str {
        match self {
            Self::Success => "200 OK",
            Self::Failure => "400 Bad Request",
            Self::NotFound => "404 Not Found",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Success => "Autorizzazione completata: puoi chiudere questa finestra e tornare all'app.",
            Self::Failure => "Autorizzazione non riuscita: chiudi questa finestra e riprova dall'app.",
            Self::NotFound => "Pagina non trovata.",
        }
    }
}

/// La risposta HTTP/1.1 completa, con una pagina statica.
pub fn response(page: Page) -> Vec<u8> {
    let body = format!(
        "<!DOCTYPE html><html lang=\"it\"><head><meta charset=\"utf-8\"><title>Muse</title></head>\
         <body><p>{}</p></body></html>",
        page.message()
    );
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\n\
         Connection: close\r\nContent-Length: {}\r\n\r\n",
        page.status(),
        body.len()
    );
    let mut out = head.into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

#[cfg(test)]
mod tests;
