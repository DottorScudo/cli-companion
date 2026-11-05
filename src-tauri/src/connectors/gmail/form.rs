//! La codifica `application/x-www-form-urlencoded` (pura): i corpi delle richieste
//! all'endpoint dei token, i parametri dell'URL di consenso e la query del redirect.

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// Tutto tranne i caratteri non riservati di RFC 3986 (`A-Za-z0-9-._~`) e' codificato.
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// `value` percent-codificato (lo spazio diventa `%20`).
pub fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, ENCODE_SET).to_string()
}

/// `a=1&b=2` con chiavi e valori codificati, come corpo di una richiesta.
pub fn form_body(pairs: &[(&str, &str)]) -> Vec<u8> {
    let joined: Vec<String> =
        pairs.iter().map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value))).collect();
    joined.join("&").into_bytes()
}

/// Il contrario di `encode_component`: `+` e' uno spazio, `%XX` e' un byte; `None` se il risultato non e' UTF-8.
pub fn decode_component(raw: &str) -> Option<String> {
    let spaced = raw.replace('+', " ");
    percent_decode_str(&spaced).decode_utf8().ok().map(std::borrow::Cow::into_owned)
}

#[cfg(test)]
mod tests;
