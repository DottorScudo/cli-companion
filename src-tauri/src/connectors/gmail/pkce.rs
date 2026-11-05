//! PKCE (RFC 7636) e `state` del consenso OAuth (puri): la casualita' la inietta il
//! chiamante (`ctx.random`), qui si trasformano solo i byte.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// Byte casuali del verifier (43 caratteri, il minimo di RFC 7636).
pub const PKCE_RANDOM_BYTES: usize = 32;
/// Byte casuali dello `state`.
pub const STATE_RANDOM_BYTES: usize = 32;
/// Lunghezza minima del verifier (RFC 7636 §4.1).
const VERIFIER_MIN: usize = 43;
/// Lunghezza massima del verifier (RFC 7636 §4.1).
const VERIFIER_MAX: usize = 128;
/// Byte casuali minimi per uno `state` non indovinabile.
const STATE_BYTES_MIN: usize = 16;

/// La coppia da usare in un consenso: il verifier resta qui, la challenge va nell'URL.
#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce").field("verifier", &"<nascosto>").field("challenge", &self.challenge).finish()
    }
}

/// `BASE64URL(SHA256(verifier))`, senza padding (metodo `S256`).
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Il verifier e' la base64url dei byte: deve venirne fuori una stringa di 43..=128 caratteri.
pub fn pkce_from_random(bytes: &[u8]) -> Option<Pkce> {
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    if !(VERIFIER_MIN..=VERIFIER_MAX).contains(&verifier.len()) {
        return None;
    }
    let challenge = challenge_for(&verifier);
    Some(Pkce { verifier, challenge })
}

/// Lo `state`: la base64url di almeno 16 byte casuali.
pub fn state_from_random(bytes: &[u8]) -> Option<String> {
    (bytes.len() >= STATE_BYTES_MIN).then(|| URL_SAFE_NO_PAD.encode(bytes))
}

/// Confronto a tempo costante sui byte; lunghezze diverse non coincidono.
pub fn states_match(expected: &str, got: &str) -> bool {
    let (a, b) = (expected.as_bytes(), got.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[cfg(test)]
mod tests;
