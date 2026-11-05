//! I set di caratteri dei corpi e delle intestazioni (puro), senza dipendenze: UTF-8 e
//! la famiglia latin1. `windows-1252` e' approssimato: i byte 0x80..=0x9F usano la
//! tabella di Windows (euro, apici tipografici...), gli altri sono come latin1.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// I caratteri dei byte 0x80..=0x9F di windows-1252 (i cinque indefiniti restano come sono).
const WINDOWS_1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’',
    '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];
/// Il primo byte coperto dalla tabella.
const WINDOWS_1252_START: u8 = 0x80;

fn windows_1252(byte: u8) -> char {
    let offset = usize::from(byte.wrapping_sub(WINDOWS_1252_START));
    match (byte >= WINDOWS_1252_START).then(|| WINDOWS_1252_HIGH.get(offset)).flatten() {
        Some(c) => *c,
        None => char::from(byte),
    }
}

/// I byte nel set di caratteri indicato; `None` se il set non e' tra quelli supportati.
pub fn decode(bytes: &[u8], charset: &str) -> Option<String> {
    let name = charset.trim().trim_matches('"').to_ascii_lowercase();
    match name.as_str() {
        "" | "utf-8" | "utf8" => Some(String::from_utf8_lossy(bytes).into_owned()),
        "iso-8859-1" | "latin1" | "latin-1" | "us-ascii" | "ascii" => {
            Some(bytes.iter().map(|b| char::from(*b)).collect())
        }
        "windows-1252" | "cp1252" => Some(bytes.iter().map(|b| windows_1252(*b)).collect()),
        _ => None,
    }
}

/// Come `decode`, ma un set sconosciuto vale UTF-8 (con i caratteri non validi sostituiti).
pub fn decode_or_utf8(bytes: &[u8], charset: &str) -> String {
    decode(bytes, charset).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

/// Base64 con alfabeto URL-safe, con o senza padding, tollerante verso gli spazi.
pub fn decode_base64url(data: &str) -> Option<Vec<u8>> {
    let compact: String = data.chars().filter(|c| !c.is_whitespace() && *c != '=').collect();
    URL_SAFE_NO_PAD.decode(compact).ok()
}

#[cfg(test)]
mod tests;
