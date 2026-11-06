//! Le sotto-chat di Telegram: i topic di un supergruppo forum, uno per thread dell'HUB.
//! Ogni chiamata aspetta il turno del gruppo nel pacer, come i messaggi.

use super::api;
use super::forum::subchat_of;
use super::pacer::Pacer;
use super::send::{parse_chat, parse_topic, wait_turn};
use crate::connectors::contracts::{ConnectorContext, ConnectorError};
use std::sync::Mutex;

/// Apre il topic `title` (tagliato al limite di Telegram) e ne ritorna l'id come sotto-chat.
pub async fn open(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    chat: &str,
    title: &str,
) -> Result<String, ConnectorError> {
    let chat_id = parse_chat(chat)?;
    wait_turn(pacer, ctx, chat).await;
    api::create_forum_topic(ctx, chat_id, title).await.map(subchat_of)
}

pub async fn rename(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    chat: &str,
    subchat: &str,
    title: &str,
) -> Result<(), ConnectorError> {
    let (chat_id, topic_id) = (parse_chat(chat)?, parse_topic(subchat)?);
    wait_turn(pacer, ctx, chat).await;
    api::edit_forum_topic(ctx, chat_id, topic_id, title).await
}

pub async fn close(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    chat: &str,
    subchat: &str,
) -> Result<(), ConnectorError> {
    let (chat_id, topic_id) = (parse_chat(chat)?, parse_topic(subchat)?);
    wait_turn(pacer, ctx, chat).await;
    api::close_forum_topic(ctx, chat_id, topic_id).await
}

pub async fn delete(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    chat: &str,
    subchat: &str,
) -> Result<(), ConnectorError> {
    let (chat_id, topic_id) = (parse_chat(chat)?, parse_topic(subchat)?);
    wait_turn(pacer, ctx, chat).await;
    api::delete_forum_topic(ctx, chat_id, topic_id).await
}

#[cfg(test)]
mod tests;
