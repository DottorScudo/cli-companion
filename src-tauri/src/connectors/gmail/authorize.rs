//! L'azione `Autorizza`: consenso OAuth «desktop» con PKCE e redirect su loopback. Si
//! controllano le credenziali prima di aprire qualunque cosa; il listener si chiude appena
//! arriva il codice (o scade l'attesa). Codice, verifier e token non escono mai da qui:
//! i messaggi di errore sono fissi.

use super::access::{client, remember, token_request};
use super::api::fetch_profile;
use super::consent::{consent_url, redirect_uri};
use super::keys::REFRESH_TOKEN_SLOT;
use super::pkce::{pkce_from_random, state_from_random, Pkce, PKCE_RANDOM_BYTES, STATE_RANDOM_BYTES};
use super::profile::authorize_outcome;
use super::token::{code_exchange_body, grants_scope, TokenGrant};
use super::tokens::TokenCache;
use super::{browser, loopback};
use crate::connectors::contracts::{ActionOutcome, ConnectorContext, ConnectorError};
use std::sync::Mutex;
use tokio::net::TcpListener;

/// Solo il loopback: il redirect non esce dalla macchina.
const LOOPBACK_HOST: &str = "127.0.0.1";
/// Porta 0: la sceglie il sistema (Google accetta qualunque porta per le app desktop).
const ANY_PORT: u16 = 0;

const NO_REFRESH_TOKEN: &str = "Google non ha rilasciato il refresh token: revoca l'accesso di questa app dalle \
    impostazioni di sicurezza dell'account Google e riprova";

fn fatal(message: &str) -> ConnectorError {
    ConnectorError::Fatal(message.to_string())
}

async fn bind() -> Result<(TcpListener, String), ConnectorError> {
    let listener = TcpListener::bind((LOOPBACK_HOST, ANY_PORT))
        .await
        .map_err(|_| fatal("impossibile mettersi in ascolto sul loopback"))?;
    let port = listener.local_addr().map_err(|_| fatal("porta di ascolto non disponibile"))?.port();
    Ok((listener, redirect_uri(port)))
}

fn new_pkce(ctx: &ConnectorContext) -> Result<Pkce, ConnectorError> {
    pkce_from_random(&ctx.random_bytes(PKCE_RANDOM_BYTES)?).ok_or_else(|| fatal("casualità non valida"))
}

fn new_state(ctx: &ConnectorContext) -> Result<String, ConnectorError> {
    state_from_random(&ctx.random_bytes(STATE_RANDOM_BYTES)?).ok_or_else(|| fatal("casualità non valida"))
}

/// Lo scambio del codice: serve il refresh token e lo scope di lettura.
async fn exchange(
    ctx: &ConnectorContext,
    code: &str,
    credentials: &(String, String),
    redirect: &str,
    pkce: &Pkce,
) -> Result<TokenGrant, ConnectorError> {
    let body = code_exchange_body(code, &credentials.0, &credentials.1, redirect, &pkce.verifier);
    let grant = token_request(ctx, body).await?;
    if grant.refresh_token.is_none() {
        return Err(fatal(NO_REFRESH_TOKEN));
    }
    if !grants_scope(&grant) {
        return Err(fatal("permesso di lettura della casella non concesso"));
    }
    Ok(grant)
}

/// Apre il browser, aspetta il consenso e salva il refresh token. Ritorna l'account autorizzato.
pub async fn authorize(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) -> Result<ActionOutcome, ConnectorError> {
    let credentials = client(ctx)?;
    let (listener, redirect) = bind().await?;
    let (pkce, state) = (new_pkce(ctx)?, new_state(ctx)?);
    browser::open(&consent_url(&credentials.0, &redirect, &state, &pkce.challenge))?;
    let code = loopback::wait_for_code(&listener, &state).await;
    drop(listener);
    let grant = exchange(ctx, &code?, &credentials, &redirect, &pkce).await?;
    if let Some(refresh_token) = grant.refresh_token.as_deref() {
        ctx.set_secret(REFRESH_TOKEN_SLOT, refresh_token)?;
    }
    remember(cache, ctx, &grant);
    let profile = fetch_profile(cache, ctx).await?;
    Ok(authorize_outcome(&profile))
}
