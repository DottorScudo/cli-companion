//! L'URL della pagina di consenso OAuth e il redirect su loopback (puri).

use super::form::encode_component;
use super::keys::{ACCOUNTS_HOST, SCOPE};

/// L'indirizzo a cui Google rimanda l'utente: il server in ascolto sul loopback.
pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// L'URL del consenso: PKCE `S256`, accesso offline (serve il refresh token) e
/// `prompt=consent` (Google rilascia il refresh token anche a un secondo consenso).
pub fn consent_url(client_id: &str, redirect_uri: &str, state: &str, challenge: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    let query: Vec<String> = params.iter().map(|(key, value)| format!("{key}={}", encode_component(value))).collect();
    format!("https://{ACCOUNTS_HOST}/o/oauth2/v2/auth?{}", query.join("&"))
}

#[cfg(test)]
mod tests;
