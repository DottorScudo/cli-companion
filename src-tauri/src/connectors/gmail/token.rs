//! L'endpoint dei token OAuth di Google (puro): i corpi delle richieste e la lettura
//! della risposta. I messaggi di errore sono fissi: mai il corpo, un token o un URL.

use super::ascii::is_visible_ascii;
use super::form::form_body;
use super::keys::SCOPE;
use crate::connectors::contracts::ConnectorError;
use serde_json::Value;
use std::fmt;

/// L'URL dell'endpoint dei token.
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Lunghezza massima di un token (di accesso o di refresh) accettato.
const TOKEN_MAX: usize = 4096;
/// Durata assunta per un token di accesso senza `expires_in`.
const EXPIRES_DEFAULT_SECS: u64 = 3600;
/// Durata massima creduta a un token di accesso: un giorno.
const EXPIRES_MAX_SECS: u64 = 24 * 60 * 60;
/// Fine delle risposte riuscite.
const STATUS_OK_MAX: u16 = 299;
const STATUS_TOO_MANY: u16 = 429;
const STATUS_CLIENT_ERROR_MIN: u16 = 400;
const STATUS_CLIENT_ERROR_MAX: u16 = 499;

/// Il corpo dello scambio del codice di autorizzazione.
pub fn code_exchange_body(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Vec<u8> {
    form_body(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ])
}

/// Il corpo del rinnovo del token di accesso.
pub fn refresh_body(refresh_token: &str, client_id: &str, client_secret: &str) -> Vec<u8> {
    form_body(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ])
}

/// Cio' che l'endpoint ha concesso. `Debug` non mostra mai i token.
#[derive(Clone, PartialEq, Eq)]
pub struct TokenGrant {
    pub access_token: String,
    pub expires_in_secs: u64,
    /// Presente solo allo scambio del codice (e se Google decide di ruotarlo).
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl fmt::Debug for TokenGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenGrant")
            .field("access_token", &"<nascosto>")
            .field("expires_in_secs", &self.expires_in_secs)
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<nascosto>"))
            .field("scope", &self.scope)
            .finish()
    }
}

fn invalid_response() -> ConnectorError {
    ConnectorError::Transient("risposta di Google non valida".to_string())
}

/// Un token valido: ASCII visibile, 1..=4096 caratteri.
fn token_field(parsed: &Value, key: &str) -> Result<Option<String>, ConnectorError> {
    match parsed.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(token)) if is_visible_ascii(token, TOKEN_MAX) => Ok(Some(token.clone())),
        Some(_) => Err(invalid_response()),
    }
}

fn grant_of(body: &[u8]) -> Result<TokenGrant, ConnectorError> {
    let parsed: Value = serde_json::from_slice(body).map_err(|_| invalid_response())?;
    let access_token = token_field(&parsed, "access_token")?.ok_or_else(invalid_response)?;
    let refresh_token = token_field(&parsed, "refresh_token")?;
    let expires_in_secs =
        parsed.get("expires_in").and_then(Value::as_u64).unwrap_or(EXPIRES_DEFAULT_SECS).min(EXPIRES_MAX_SECS);
    let scope = parsed.get("scope").and_then(Value::as_str).map(str::to_string);
    Ok(TokenGrant { access_token, expires_in_secs, refresh_token, scope })
}

/// Il nome dell'errore OAuth (`error`) di una risposta non riuscita.
fn oauth_error(body: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(body).ok()?;
    parsed.get("error")?.as_str().map(str::to_string)
}

fn error_of(status: u16, body: &[u8]) -> ConnectorError {
    if status == STATUS_TOO_MANY {
        return ConnectorError::RateLimited { retry_after: None };
    }
    if !(STATUS_CLIENT_ERROR_MIN..=STATUS_CLIENT_ERROR_MAX).contains(&status) {
        return ConnectorError::Transient("Google non ha risposto come previsto".to_string());
    }
    match oauth_error(body).as_deref() {
        Some("invalid_grant") => ConnectorError::Auth("autorizzazione revocata o scaduta".to_string()),
        Some("invalid_client" | "unauthorized_client") => ConnectorError::Auth("client OAuth rifiutato".to_string()),
        _ => ConnectorError::Fatal("richiesta di autorizzazione rifiutata da Google".to_string()),
    }
}

/// La risposta dell'endpoint dei token come concessione o come errore classificato.
pub fn parse_token_response(status: u16, body: &[u8]) -> Result<TokenGrant, ConnectorError> {
    if (200..=STATUS_OK_MAX).contains(&status) {
        grant_of(body)
    } else {
        Err(error_of(status, body))
    }
}

/// La concessione comprende lo scope di lettura? Se Google non elenca gli scope si assume di si'.
pub fn grants_scope(grant: &TokenGrant) -> bool {
    grant.scope.as_deref().is_none_or(|scope| scope.split_whitespace().any(|s| s == SCOPE))
}

#[cfg(test)]
mod tests;
