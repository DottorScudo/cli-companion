//! Le parole codificate di RFC 2047 nelle intestazioni (`=?utf-8?B?...?=`, `?Q?`).
//! Serve perche' l'API restituisce Subject e From come arrivano nel messaggio. Set di
//! caratteri supportati: quelli di `charset`; una parola con altro resta com'e'.

use super::charset::decode;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;

const WORD_START: &str = "=?";
const WORD_END: &str = "?=";

/// I byte di una parola `Q`: `_` e' uno spazio, `=XX` un byte. `None` se un escape e' rotto.
fn decode_q(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'_' => out.push(b' '),
            b'=' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 2;
            }
            other => out.push(other),
        }
        i += 1;
    }
    Some(out)
}

fn decode_b(text: &str) -> Option<Vec<u8>> {
    STANDARD_NO_PAD.decode(text.trim_end_matches('=')).ok()
}

/// Una parola che inizia a `s` (`=?charset?E?testo?=`): il testo decodificato e quanti byte occupa.
fn parse_word(s: &str) -> Option<(String, usize)> {
    let inner = s.strip_prefix(WORD_START)?;
    let (charset, rest) = inner.split_once('?')?;
    let (encoding, rest) = rest.split_once('?')?;
    let end = rest.find(WORD_END)?;
    let text = rest.get(..end)?;
    let bytes = match encoding {
        "B" | "b" => decode_b(text)?,
        "Q" | "q" => decode_q(text)?,
        _ => return None,
    };
    let charset = charset.split('*').next().unwrap_or(charset);
    let decoded = decode(&bytes, charset)?;
    let consumed = s.len().saturating_sub(rest.len()).saturating_add(end).saturating_add(WORD_END.len());
    Some((decoded, consumed))
}

/// `header` con le parole codificate decodificate; fra due parole adiacenti gli spazi spariscono (RFC 2047 §6.2).
pub fn decode_words(header: &str) -> String {
    let mut out = String::with_capacity(header.len());
    let mut rest = header;
    let mut after_word = false;
    while let Some(at) = rest.find(WORD_START) {
        let (before, tail) = rest.split_at(at);
        match parse_word(tail) {
            Some((decoded, consumed)) => {
                if !(after_word && before.chars().all(char::is_whitespace)) {
                    out.push_str(before);
                }
                out.push_str(&decoded);
                rest = tail.get(consumed..).unwrap_or("");
                after_word = true;
            }
            None => {
                out.push_str(before);
                out.push_str(WORD_START);
                rest = tail.get(WORD_START.len()..).unwrap_or("");
                after_word = false;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
