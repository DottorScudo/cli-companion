//! Le chiamate GET alla Gmail API: token `Bearer`, classificazione della risposta e JSON.
//! Un 401 invalida il token in cache e si riprova UNA volta con uno nuovo; un secondo
//! 401 e' un'autorizzazione da rifare. Gli errori sono fissi: mai URL o corpi.

use super::access::{access_token, forget};
use super::api_urls::profile_url;
use super::classify::{classify, ApiFailure};
use super::profile::{parse_profile, Profile};
use super::tokens::TokenCache;
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use crate::connectors::limits::RESPONSE_MAX;
use crate::connectors::runtime::http::HttpRequest;
use crate::hub::attachments::ATTACHMENT_SIZE_MAX;
use serde_json::Value;
use std::sync::Mutex;

/// Tetto della risposta di un messaggio intero (`format=full`).
pub const MESSAGE_RESPONSE_MAX: usize = 5 * 1024 * 1024;
/// Margine per la busta JSON (`size`, `attachmentId`) attorno ai dati di un allegato.
const ATTACHMENT_ENVELOPE_MARGIN: usize = 64 * 1024;
/// Tetto della risposta di un allegato: i dati sono in base64 (4 caratteri ogni 3 byte, per
/// eccesso), quindi un allegato al limite dell'HUB pesa circa 13,4 MB: piu' dei 10,25 MB di
/// `RESPONSE_MAX_FILES`, pensato per i file grezzi.
pub const ATTACHMENT_RESPONSE_MAX: usize = (ATTACHMENT_SIZE_MAX as usize).div_ceil(3) * 4 + ATTACHMENT_ENVELOPE_MARGIN;
/// Tetto delle risposte normali (profilo, elenchi).
pub const NORMAL_RESPONSE_MAX: usize = RESPONSE_MAX;

const _: () = assert!(ATTACHMENT_RESPONSE_MAX > (ATTACHMENT_SIZE_MAX as usize).div_ceil(3) * 4);

fn unauthorized() -> ApiFailure {
    ApiFailure::Error(ConnectorError::Auth("accesso a Gmail non autorizzato".to_string()))
}

async fn attempt(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    url: &str,
    max_body: usize,
) -> Result<Value, ApiFailure> {
    let token = access_token(cache, ctx).await.map_err(ApiFailure::Error)?;
    let request = HttpRequest::get(url).header("authorization", &format!("Bearer {token}"));
    let response = ctx
        .http()
        .clone()
        .with_max_body(max_body)
        .send(request)
        .await
        .map_err(|e| ApiFailure::Error(ConnectorError::from(e)))?;
    classify(response.status, response.header("retry-after"), &response.body)?;
    serde_json::from_slice(&response.body)
        .map_err(|_| ApiFailure::Error(ConnectorError::Transient("risposta di Gmail non valida".to_string())))
}

/// `GET url` come JSON, con la risposta limitata a `max_body` byte.
pub async fn get_json(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    url: &str,
    max_body: usize,
) -> Result<Value, ApiFailure> {
    match attempt(cache, ctx, url, max_body).await {
        Err(ApiFailure::Unauthorized) => {
            forget(cache, ctx);
            match attempt(cache, ctx, url, max_body).await {
                Err(ApiFailure::Unauthorized) => Err(unauthorized()),
                other => other,
            }
        }
        other => other,
    }
}

/// L'errore del connettore di un insuccesso che il chiamante non gestisce.
pub fn into_error(failure: ApiFailure) -> ConnectorError {
    match failure {
        ApiFailure::Error(e) => e,
        ApiFailure::Unauthorized => ConnectorError::Auth("accesso a Gmail non autorizzato".to_string()),
        ApiFailure::NotFound | ApiFailure::BadRequest => {
            ConnectorError::Transient("Gmail non ha accettato la richiesta".to_string())
        }
    }
}

/// Come `get_json` per chi non distingue 404 e 400: sono errori transitori.
pub async fn get_value(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    url: &str,
    max_body: usize,
) -> Result<Value, ConnectorError> {
    get_json(cache, ctx, url, max_body).await.map_err(into_error)
}

/// Il profilo dell'account autorizzato (`getProfile`).
pub async fn fetch_profile(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) -> Result<Profile, ConnectorError> {
    parse_profile(&get_value(cache, ctx, &profile_url(), NORMAL_RESPONSE_MAX).await?)
}
