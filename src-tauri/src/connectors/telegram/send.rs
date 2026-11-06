//! L'invio di un messaggio a una chat (o a un topic di un forum): il testo spezzato,
//! la cadenza per chat e la risposta legata al messaggio d'origine solo sulla prima parte.

use super::forum::topic_id;
use super::ids::{external_id, reply_message_id};
use super::pacer::Pacer;
use super::requests::Destination;
use super::{api, text};
use crate::connectors::contracts::{ConnectorContext, ConnectorError, OutboundMessage};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// Attesa massima prima di un messaggio, qualunque cosa dica l'orologio.
const WAIT_MAX: Duration = Duration::from_secs(30);

/// Prenota il posto per `chat` (senza tenere il lucchetto oltre la prenotazione) e ritorna l'attesa.
fn reserve_slot(pacer: &Mutex<Pacer>, chat: &str, now_ms: i64) -> Duration {
    let wait_ms = pacer.lock().unwrap_or_else(PoisonError::into_inner).reserve(chat, now_ms);
    Duration::from_millis(u64::try_from(wait_ms).unwrap_or(0)).min(WAIT_MAX)
}

/// Aspetta il turno di `chat` prima di una chiamata che vi scrive.
pub async fn wait_turn(pacer: &Mutex<Pacer>, ctx: &ConnectorContext, chat: &str) {
    let wait = reserve_slot(pacer, chat, ctx.now());
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
}

pub fn parse_chat(chat: &str) -> Result<i64, ConnectorError> {
    chat.parse().map_err(|_| ConnectorError::Fatal("chat non valida".to_string()))
}

/// L'id del topic di una sotto-chat. Un id che non e' di un topic (lo stato e' stato
/// scritto da altri) vale come un topic sparito: il runtime toglie la voce e lo riapre.
pub fn parse_topic(subchat: &str) -> Result<i64, ConnectorError> {
    topic_id(subchat).ok_or(ConnectorError::SubchatGone)
}

/// Dove va il messaggio: la chat e, se c'e' la sotto-chat, il suo topic.
fn destination(out: &OutboundMessage) -> Result<Destination, ConnectorError> {
    let chat_id = parse_chat(&out.chat)?;
    let topic_id = out.subchat.as_deref().map(parse_topic).transpose()?;
    Ok(Destination { chat_id, topic_id })
}

/// Manda il testo e ritorna l'id esterno di ogni parte, in ordine. Vuoto per un testo vuoto.
pub async fn send(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    out: &OutboundMessage,
) -> Result<Vec<String>, ConnectorError> {
    let to = destination(out)?;
    let mut reply_to = reply_message_id(&out.chat, out.reply_to_external.as_deref());
    let mut sent = Vec::new();
    for part in text::split(&out.text) {
        wait_turn(pacer, ctx, &out.chat).await;
        let message_id = api::send_message(ctx, to, &part, reply_to.take()).await?;
        sent.push(external_id(to.chat_id, message_id));
    }
    Ok(sent)
}

#[cfg(test)]
mod tests;
