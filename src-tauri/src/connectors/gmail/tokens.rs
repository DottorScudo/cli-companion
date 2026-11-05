//! La cache dei token di accesso (pura): per istanza, finche' non scadono. Sta in una
//! `static` dietro un `Mutex`, cosi' un giro non rinnova il token a ogni chiamata.

use super::token::TokenGrant;
use crate::connectors::limits::MAX_INSTANCES;
use std::collections::BTreeMap;
use std::fmt;

/// Margine prima della scadenza: un token cosi' vicino alla fine non si usa piu'.
pub const EXPIRY_MARGIN_MS: i64 = 60_000;
/// Millisecondi in un secondo.
const MS_PER_SEC: i64 = 1000;

/// Un token di accesso e il momento in cui scade (millisecondi Unix).
#[derive(Clone)]
struct CachedToken {
    value: String,
    expires_at_ms: i64,
}

impl fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedToken").field("value", &"<nascosto>").field("expires_at_ms", &self.expires_at_ms).finish()
    }
}

/// Instance id -> token di accesso. Al piu' `MAX_INSTANCES` voci.
#[derive(Debug, Default)]
pub struct TokenCache {
    entries: BTreeMap<String, CachedToken>,
}

impl TokenCache {
    pub const fn new() -> Self {
        Self { entries: BTreeMap::new() }
    }

    /// Il token dell'istanza, se vale ancora per almeno `EXPIRY_MARGIN_MS`.
    pub fn get(&self, instance: &str, now_ms: i64) -> Option<String> {
        let cached = self.entries.get(instance)?;
        (cached.expires_at_ms.saturating_sub(EXPIRY_MARGIN_MS) > now_ms).then(|| cached.value.clone())
    }

    /// Ricorda il token concesso; se le voci superano il tetto esce quella che scade prima.
    pub fn put(&mut self, instance: &str, grant: &TokenGrant, now_ms: i64) {
        let lifetime_ms = i64::try_from(grant.expires_in_secs).unwrap_or(i64::MAX).saturating_mul(MS_PER_SEC);
        let token =
            CachedToken { value: grant.access_token.clone(), expires_at_ms: now_ms.saturating_add(lifetime_ms) };
        self.entries.insert(instance.to_string(), token);
        while self.entries.len() > MAX_INSTANCES {
            let soonest = self
                .entries
                .iter()
                .filter(|(id, _)| id.as_str() != instance)
                .min_by_key(|(_, cached)| cached.expires_at_ms)
                .map(|(id, _)| id.clone());
            match soonest {
                Some(id) => self.entries.remove(&id),
                None => break,
            };
        }
    }

    /// Dimentica il token (un 401: era stato revocato).
    pub fn invalidate(&mut self, instance: &str) {
        self.entries.remove(instance);
    }
}

#[cfg(test)]
mod tests;
