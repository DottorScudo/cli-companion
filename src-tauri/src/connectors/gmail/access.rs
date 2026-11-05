//! Il token di accesso: dalla cache o rinnovato col refresh token. Le credenziali del
//! client (id in configurazione, secret nel portachiavi) si leggono qui. I messaggi
//! di errore sono fissi: mai un token, un secret o il corpo di una risposta.

use super::keys::{CLIENT_ID_KEY, CLIENT_SECRET_SLOT, REFRESH_TOKEN_SLOT};
use super::token::{parse_token_response, refresh_body, TokenGrant, TOKEN_URL};
use super::tokens::TokenCache;
use super::validate::validate_client_id;
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use crate::connectors::runtime::http::HttpRequest;
use serde_json::Value;
use std::sync::{Mutex, MutexGuard, PoisonError};

const FORM_CONTENT_TYPE: (&str, &str) = ("content-type", "application/x-www-form-urlencoded");

fn locked(cache: &Mutex<TokenCache>) -> MutexGuard<'_, TokenCache> {
    cache.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Un segreto non vuoto, senza spazi ai bordi.
fn secret(ctx: &ConnectorContext, slot: &str) -> Result<Option<String>, ConnectorError> {
    Ok(ctx.secret(slot)?.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

/// Client id e client secret. Un id mancante o malformato e' un errore di configurazione,
/// un secret mancante un'autorizzazione da rifare.
pub fn client(ctx: &ConnectorContext) -> Result<(String, String), ConnectorError> {
    let client_id = ctx
        .config()
        .get(CLIENT_ID_KEY)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| validate_client_id(id).is_ok())
        .ok_or_else(|| ConnectorError::Config("client ID OAuth mancante o non valido".to_string()))?;
    let client_secret = secret(ctx, CLIENT_SECRET_SLOT)?
        .ok_or_else(|| ConnectorError::Auth("client secret OAuth mancante".to_string()))?;
    Ok((client_id.to_string(), client_secret))
}

/// Una richiesta all'endpoint dei token.
pub async fn token_request(ctx: &ConnectorContext, body: Vec<u8>) -> Result<TokenGrant, ConnectorError> {
    let request = HttpRequest::post(TOKEN_URL, body).header(FORM_CONTENT_TYPE.0, FORM_CONTENT_TYPE.1);
    let response = ctx.http().send(request).await?;
    parse_token_response(response.status, &response.body)
}

/// Mette in cache il token concesso (il lucchetto non attraversa mai un `.await`).
pub fn remember(cache: &Mutex<TokenCache>, ctx: &ConnectorContext, grant: &TokenGrant) {
    locked(cache).put(ctx.instance(), grant, ctx.now());
}

/// Dimentica il token dell'istanza (un 401).
pub fn forget(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) {
    locked(cache).invalidate(ctx.instance());
}

async fn renew(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) -> Result<String, ConnectorError> {
    let refresh_token =
        secret(ctx, REFRESH_TOKEN_SLOT)?.ok_or_else(|| ConnectorError::Auth("autorizzazione mancante".to_string()))?;
    let (client_id, client_secret) = client(ctx)?;
    let grant = token_request(ctx, refresh_body(&refresh_token, &client_id, &client_secret)).await?;
    remember(cache, ctx, &grant);
    if let Some(rotated) = grant.refresh_token.as_deref() {
        ctx.set_secret(REFRESH_TOKEN_SLOT, rotated)?;
    }
    Ok(grant.access_token)
}

/// Il token di accesso dell'istanza: quello in cache se vale ancora, altrimenti uno nuovo.
pub async fn access_token(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) -> Result<String, ConnectorError> {
    let cached = locked(cache).get(ctx.instance(), ctx.now());
    match cached {
        Some(token) => Ok(token),
        None => renew(cache, ctx).await,
    }
}

#[cfg(test)]
mod tests;
