//! La risposta di `getProfile` e gli esiti delle azioni `Prova` e `Autorizza` (puri).

use crate::connectors::contracts::{ActionOutcome, ConnectorError};
use serde_json::Value;

/// Lunghezza minima e massima di un indirizzo email (RFC 5321).
const EMAIL_MIN: usize = 3;
const EMAIL_MAX: usize = 254;

/// L'account autorizzato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub email: String,
}

fn plausible_email(email: &str) -> bool {
    (EMAIL_MIN..=EMAIL_MAX).contains(&email.len())
        && email.contains('@')
        && !email.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Legge `emailAddress`.
pub fn parse_profile(v: &Value) -> Result<Profile, ConnectorError> {
    match v.get("emailAddress").and_then(Value::as_str) {
        Some(email) if plausible_email(email) => Ok(Profile { email: email.to_string() }),
        _ => Err(ConnectorError::Transient("profilo di Gmail non valido".to_string())),
    }
}

/// L'esito di `Prova`.
pub fn test_outcome(profile: &Profile) -> ActionOutcome {
    ActionOutcome {
        message: format!("Account Gmail raggiungibile: {}.", profile.email),
        detail: Some(profile.email.clone()),
    }
}

/// L'esito di `Autorizza`.
pub fn authorize_outcome(profile: &Profile) -> ActionOutcome {
    ActionOutcome {
        message: format!("Autorizzazione completata per {}.", profile.email),
        detail: Some(profile.email.clone()),
    }
}

#[cfg(test)]
mod tests;
