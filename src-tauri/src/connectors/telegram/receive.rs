//! Un giro di long polling su `getUpdates`. Al primo giro (nessun cursore) si parte
//! da "adesso": si toglie un webhook eventuale e si salta lo storico. Nei log solo
//! l'id dell'istanza, i tipi e i conteggi: mai id di chat, nomi o testi.

use super::requests::{latest_update_body, poll_body, POLL_WAIT_SECS};
use super::updates::{next_offset, parse_updates, Update, UpdateKind};
use super::{api, cursor, inbound};
use crate::connectors::contracts::{ConnectorContext, ConnectorError, ReceiveBatch};
use crate::connectors::limits::{CALL_TIMEOUT, LONG_POLL_MAX, RECEIVE_TIMEOUT};
use serde_json::Value;
use std::time::Duration;

/// Margine fra l'attesa di Telegram e il timeout HTTP del long polling.
const POLL_HTTP_MARGIN: Duration = Duration::from_secs(10);
/// Il timeout HTTP di un giro di attesa.
const POLL_HTTP_TIMEOUT: Duration = Duration::from_secs(POLL_WAIT_SECS + POLL_HTTP_MARGIN.as_secs());

const _: () = assert!(POLL_WAIT_SECS <= LONG_POLL_MAX.as_secs());
const _: () = assert!(POLL_HTTP_TIMEOUT.as_secs() < RECEIVE_TIMEOUT.as_secs());

/// Il primo giro: niente storico, il cursore riparte dall'ultimo aggiornamento.
async fn start_from_now(ctx: &ConnectorContext) -> Result<ReceiveBatch, ConnectorError> {
    api::delete_webhook(ctx).await?;
    let result = api::get_updates(ctx, &latest_update_body(), CALL_TIMEOUT).await?;
    let offset = next_offset(&parse_updates(&result)).unwrap_or(0);
    log::info!("connettori {}: telegram: partenza da adesso", ctx.instance());
    Ok(ReceiveBatch { messages: Vec::new(), cursor: cursor::to_value(offset) })
}

/// Esce dalle chat non private in cui il bot e' stato aggiunto. Un errore si registra
/// per classe e non interrompe il giro.
async fn leave_unwanted_chats(ctx: &ConnectorContext, updates: &[Update]) {
    for update in updates {
        if let UpdateKind::JoinedChat(chat_id) = update.kind {
            match api::leave_chat(ctx, chat_id).await {
                Ok(()) => log::info!("connettori {}: telegram: uscito da una chat non privata", ctx.instance()),
                Err(e) => log::warn!(
                    "connettori {}: telegram: uscita da una chat non privata non riuscita ({})",
                    ctx.instance(),
                    e.class()
                ),
            }
        }
    }
}

fn log_ignored(ctx: &ConnectorContext, updates: &[Update]) {
    for update in updates {
        if let UpdateKind::Ignored(kind) = update.kind {
            log::info!("connettori {}: telegram: aggiornamento ignorato ({kind})", ctx.instance());
        }
    }
}

/// Un giro di attesa a partire da `offset`.
async fn poll(ctx: &ConnectorContext, offset: i64) -> Result<ReceiveBatch, ConnectorError> {
    let result = api::get_updates(ctx, &poll_body(offset), POLL_HTTP_TIMEOUT).await?;
    let updates = parse_updates(&result);
    log_ignored(ctx, &updates);
    leave_unwanted_chats(ctx, &updates).await;
    let messages = inbound::build_messages(&updates);
    log::info!("connettori {}: telegram: {} aggiornamenti, {} messaggi", ctx.instance(), updates.len(), messages.len());
    let next = next_offset(&updates).map_or(offset, |next| next.max(offset));
    Ok(ReceiveBatch { messages, cursor: cursor::to_value(next) })
}

pub async fn receive(ctx: &ConnectorContext, saved: Option<Value>) -> Result<ReceiveBatch, ConnectorError> {
    match cursor::parse(saved.as_ref()) {
        Some(offset) => poll(ctx, offset).await,
        None => start_from_now(ctx).await,
    }
}

#[cfg(test)]
mod tests;
