//! Il token del bot: formato (pura), lettura dal portachiavi e URL della Bot API.
//! Il token sta nel percorso dell'URL: non va mai in log, errori, DTO o stato. I
//! messaggi di errore sono fissi.

use super::descriptor::{API_HOST, TOKEN_SLOT};
use crate::connectors::contracts::{ConnectorContext, ConnectorError};

/// Lunghezza minima della parte segreta del token (dopo i due punti).
const SECRET_MIN: usize = 30;
/// Tetto prudente alla parte segreta, per non costruire URL enormi.
const SECRET_MAX: usize = 128;
/// Tetto alle cifre dell'id del bot.
const ID_DIGITS_MAX: usize = 20;

/// `<cifre>:<[A-Za-z0-9_-]{30,}>`.
pub fn valid_token(token: &str) -> bool {
    let Some((id, secret)) = token.split_once(':') else { return false };
    let id_ok = !id.is_empty() && id.len() <= ID_DIGITS_MAX && id.bytes().all(|b| b.is_ascii_digit());
    let secret_ok = (SECRET_MIN..=SECRET_MAX).contains(&secret.len())
        && secret.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    id_ok && secret_ok
}

/// Il token validato dell'istanza. Mancante o malformato: `Auth` (serve l'utente).
pub fn bot_token(ctx: &ConnectorContext) -> Result<String, ConnectorError> {
    match ctx.secret(TOKEN_SLOT)? {
        None => Err(ConnectorError::Auth("token del bot mancante".to_string())),
        Some(token) if valid_token(token.trim()) => Ok(token.trim().to_string()),
        Some(_) => Err(ConnectorError::Auth("token del bot non valido".to_string())),
    }
}

/// `https://api.telegram.org/bot<token>/<method>`.
pub fn method_url(token: &str, method: &str) -> String {
    format!("https://{API_HOST}/bot{token}/{method}")
}

/// `https://api.telegram.org/file/bot<token>/<file_path>`: `file_path` va validato prima.
pub fn file_url(token: &str, file_path: &str) -> String {
    format!("https://{API_HOST}/file/bot{token}/{file_path}")
}

#[cfg(test)]
mod tests;
