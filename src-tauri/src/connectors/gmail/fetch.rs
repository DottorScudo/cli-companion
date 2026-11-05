//! Lo scarico di un messaggio e dei suoi allegati. Un messaggio sparito o illeggibile non
//! ferma il giro; gli errori del servizio (rete, limiti, autorizzazione) si propagano.

use super::api::{get_json, into_error, ATTACHMENT_RESPONSE_MAX, MESSAGE_RESPONSE_MAX};
use super::api_urls::{attachment_url, message_url};
use super::attachments::{display_name, left_out, Files, Selection};
use super::charset::decode_base64url;
use super::classify::ApiFailure;
use super::message::{parse_message, ParsedMessage};
use super::mime::{AttachmentRef, AttachmentSource};
use super::tokens::TokenCache;
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use crate::hub::attachments::{IncomingFile, ATTACHMENT_SIZE_MAX};
use serde_json::Value;
use std::sync::Mutex;

/// Cosa e' venuto fuori dalla lettura di un messaggio.
#[derive(Debug, PartialEq, Eq)]
pub enum Fetched {
    /// Cancellato nel frattempo.
    Gone,
    /// In una forma che non si capisce: si salta.
    Unreadable,
    Message(Box<ParsedMessage>),
}

/// Un allegato che non si riesce a leggere resta non importato; un errore del servizio (limiti,
/// autorizzazione) si propaga.
fn attachment_failure(failure: ApiFailure) -> Result<Option<Vec<u8>>, ConnectorError> {
    match failure {
        ApiFailure::NotFound | ApiFailure::BadRequest | ApiFailure::Error(ConnectorError::Transient(_)) => Ok(None),
        other => Err(into_error(other)),
    }
}

/// Legge un messaggio intero.
pub async fn message(cache: &Mutex<TokenCache>, ctx: &ConnectorContext, id: &str) -> Result<Fetched, ConnectorError> {
    let Some(url) = message_url(id) else {
        return Ok(Fetched::Unreadable);
    };
    match get_json(cache, ctx, &url, MESSAGE_RESPONSE_MAX).await {
        Ok(body) => Ok(parse_message(&body).map_or(Fetched::Unreadable, |m| Fetched::Message(Box::new(m)))),
        Err(ApiFailure::NotFound) => Ok(Fetched::Gone),
        Err(ApiFailure::BadRequest) => Ok(Fetched::Unreadable),
        Err(other) => Err(into_error(other)),
    }
}

/// I byte di un allegato `Remote`; `None` se non si puo' scaricare (sparito, troppo grande, illeggibile).
async fn remote_bytes(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    message_id: &str,
    attachment_id: &str,
) -> Result<Option<Vec<u8>>, ConnectorError> {
    let Some(url) = attachment_url(message_id, attachment_id) else {
        return Ok(None);
    };
    match get_json(cache, ctx, &url, ATTACHMENT_RESPONSE_MAX).await {
        Ok(body) => Ok(decoded(&body)),
        Err(failure) => attachment_failure(failure),
    }
}

/// `data` (base64url) decodificato, se entro il limite dell'HUB.
fn decoded(body: &Value) -> Option<Vec<u8>> {
    let bytes = decode_base64url(body.get("data")?.as_str()?)?;
    (bytes.len() as u64 <= ATTACHMENT_SIZE_MAX).then_some(bytes)
}

async fn bytes_of(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    message_id: &str,
    attachment: &AttachmentRef,
) -> Result<Option<Vec<u8>>, ConnectorError> {
    match &attachment.source {
        AttachmentSource::Inline(bytes) => Ok((bytes.len() as u64 <= ATTACHMENT_SIZE_MAX).then(|| bytes.clone())),
        AttachmentSource::Remote(id) => remote_bytes(cache, ctx, message_id, id).await,
    }
}

/// Scarica gli allegati scelti; quelli che non si riescono a scaricare vanno fra i non importati.
pub async fn download(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    message: &ParsedMessage,
    selection: Selection,
) -> Result<Files, ConnectorError> {
    let mut files = Files { files: Vec::new(), not_imported: selection.not_imported };
    for attachment in selection.wanted.iter().filter_map(|i| message.attachments.get(*i)) {
        match bytes_of(cache, ctx, &message.id, attachment).await? {
            Some(bytes) => files.files.push(IncomingFile { name: display_name(attachment), bytes }),
            None => files.not_imported.push(left_out(attachment)),
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests;
