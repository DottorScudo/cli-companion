//! Le chiamate alla Bot API sul client del contesto (`ctx.http()`, l'unico client di
//! rete di un connettore). POST JSON su `https://api.telegram.org/bot<token>/<metodo>`;
//! la risposta passa da `classify`. Gli errori hanno testi fissi: mai l'URL (ha il token).

use super::classify::{classify, entities_rejected};
use super::requests::{self, Destination, TextMode};
use super::token::{bot_token, method_url};
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::http::{HttpRequest, HttpResponse};
use serde_json::Value;
use std::time::Duration;

const CONTENT_TYPE: (&str, &str) = ("content-type", "application/json");

/// Come e' finito un `sendMessage` in HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlSent {
    /// Consegnato: l'id del messaggio.
    Delivered(i64),
    /// Telegram non ha letto l'HTML: niente e' stato consegnato.
    EntitiesRejected,
}

/// Manda `params` a `method` e restituisce la risposta grezza (ancora da classificare).
async fn post(
    ctx: &ConnectorContext,
    method: &str,
    params: &Value,
    timeout: Duration,
) -> Result<HttpResponse, ConnectorError> {
    let url = method_url(&bot_token(ctx)?, method);
    let body =
        serde_json::to_vec(params).map_err(|_| ConnectorError::Fatal("richiesta non serializzabile".to_string()))?;
    let request = HttpRequest::post(&url, body).header(CONTENT_TYPE.0, CONTENT_TYPE.1).timeout(timeout);
    Ok(ctx.http().send(request).await?)
}

/// Chiama `method` con `params` e restituisce il `result`.
pub async fn call(
    ctx: &ConnectorContext,
    method: &str,
    params: &Value,
    timeout: Duration,
) -> Result<Value, ConnectorError> {
    let response = post(ctx, method, params, timeout).await?;
    classify(response.status, &response.body)
}

async fn call_short(ctx: &ConnectorContext, method: &str, params: &Value) -> Result<Value, ConnectorError> {
    call(ctx, method, params, CALL_TIMEOUT).await
}

/// `getMe`: chi e' il bot.
pub async fn get_me(ctx: &ConnectorContext) -> Result<Value, ConnectorError> {
    call_short(ctx, "getMe", &Value::Object(serde_json::Map::new())).await
}

pub async fn delete_webhook(ctx: &ConnectorContext) -> Result<(), ConnectorError> {
    call_short(ctx, "deleteWebhook", &requests::delete_webhook_body()).await.map(drop)
}

/// `getUpdates` con il corpo e il tetto di tempo scelti dal chiamante (il long polling dura di piu').
pub async fn get_updates(ctx: &ConnectorContext, body: &Value, timeout: Duration) -> Result<Value, ConnectorError> {
    call(ctx, "getUpdates", body, timeout).await
}

pub async fn leave_chat(ctx: &ConnectorContext, chat_id: i64) -> Result<(), ConnectorError> {
    call_short(ctx, "leaveChat", &requests::leave_chat_body(chat_id)).await.map(drop)
}

pub async fn get_file(ctx: &ConnectorContext, file_id: &str) -> Result<Value, ConnectorError> {
    call_short(ctx, "getFile", &requests::get_file_body(file_id)).await
}

/// Un intero del `result` (un id), o un errore se Telegram ha risposto altro.
fn id_field(result: &Value, key: &str) -> Result<i64, ConnectorError> {
    result
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| ConnectorError::Transient("risposta di Telegram non riconosciuta".to_string()))
}

/// `sendMessage` in HTML. Un rifiuto dell'HTML (`entities_rejected`) non e' un errore:
/// il chiamante puo' rimandare il testo semplice.
pub async fn send_html_message(
    ctx: &ConnectorContext,
    to: Destination,
    html: &str,
    reply_to: Option<i64>,
) -> Result<HtmlSent, ConnectorError> {
    let body = requests::send_message_body(to, html, TextMode::Html, reply_to);
    let response = post(ctx, "sendMessage", &body, CALL_TIMEOUT).await?;
    if entities_rejected(response.status, &response.body) {
        return Ok(HtmlSent::EntitiesRejected);
    }
    let result = classify(response.status, &response.body)?;
    id_field(&result, "message_id").map(HtmlSent::Delivered)
}

/// `sendMessage` in testo semplice: l'id del messaggio inviato.
pub async fn send_plain_message(
    ctx: &ConnectorContext,
    to: Destination,
    text: &str,
    reply_to: Option<i64>,
) -> Result<i64, ConnectorError> {
    let body = requests::send_message_body(to, text, TextMode::Plain, reply_to);
    let result = call_short(ctx, "sendMessage", &body).await?;
    id_field(&result, "message_id")
}

/// `getChatMember`: lo stato di un utente (qui il bot) nella chat.
pub async fn get_chat_member(ctx: &ConnectorContext, chat_id: i64, user_id: i64) -> Result<Value, ConnectorError> {
    call_short(ctx, "getChatMember", &requests::get_chat_member_body(chat_id, user_id)).await
}

/// `createForumTopic`: l'id del topic aperto.
pub async fn create_forum_topic(ctx: &ConnectorContext, chat_id: i64, title: &str) -> Result<i64, ConnectorError> {
    let result = call_short(ctx, "createForumTopic", &requests::create_forum_topic_body(chat_id, title)).await?;
    id_field(&result, "message_thread_id")
}

pub async fn edit_forum_topic(
    ctx: &ConnectorContext,
    chat_id: i64,
    topic_id: i64,
    title: &str,
) -> Result<(), ConnectorError> {
    call_short(ctx, "editForumTopic", &requests::edit_forum_topic_body(chat_id, topic_id, title)).await.map(drop)
}

pub async fn close_forum_topic(ctx: &ConnectorContext, chat_id: i64, topic_id: i64) -> Result<(), ConnectorError> {
    call_short(ctx, "closeForumTopic", &requests::topic_body(chat_id, topic_id)).await.map(drop)
}

pub async fn reopen_forum_topic(ctx: &ConnectorContext, chat_id: i64, topic_id: i64) -> Result<(), ConnectorError> {
    call_short(ctx, "reopenForumTopic", &requests::topic_body(chat_id, topic_id)).await.map(drop)
}

pub async fn delete_forum_topic(ctx: &ConnectorContext, chat_id: i64, topic_id: i64) -> Result<(), ConnectorError> {
    call_short(ctx, "deleteForumTopic", &requests::topic_body(chat_id, topic_id)).await.map(drop)
}
