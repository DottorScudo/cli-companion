//! Le chiamate alla Bot API sul client del contesto (`ctx.http()`, l'unico client di
//! rete di un connettore). POST JSON su `https://api.telegram.org/bot<token>/<metodo>`;
//! la risposta passa da `classify`. Gli errori hanno testi fissi: mai l'URL (ha il token).

use super::classify::classify;
use super::requests;
use super::token::{bot_token, method_url};
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::http::HttpRequest;
use serde_json::Value;
use std::time::Duration;

const CONTENT_TYPE: (&str, &str) = ("content-type", "application/json");

/// Chiama `method` con `params` e restituisce il `result`.
pub async fn call(
    ctx: &ConnectorContext,
    method: &str,
    params: &Value,
    timeout: Duration,
) -> Result<Value, ConnectorError> {
    let url = method_url(&bot_token(ctx)?, method);
    let body =
        serde_json::to_vec(params).map_err(|_| ConnectorError::Fatal("richiesta non serializzabile".to_string()))?;
    let request = HttpRequest::post(&url, body).header(CONTENT_TYPE.0, CONTENT_TYPE.1).timeout(timeout);
    let response = ctx.http().send(request).await?;
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

/// `sendMessage`: l'id del messaggio inviato.
pub async fn send_message(
    ctx: &ConnectorContext,
    chat_id: i64,
    text: &str,
    reply_to: Option<i64>,
) -> Result<i64, ConnectorError> {
    let result = call_short(ctx, "sendMessage", &requests::send_message_body(chat_id, text, reply_to)).await?;
    result
        .get("message_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ConnectorError::Transient("risposta di Telegram non riconosciuta".to_string()))
}
