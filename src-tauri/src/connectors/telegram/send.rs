//! L'invio di un messaggio a una chat: il testo spezzato, la cadenza per chat e la
//! risposta legata al messaggio d'origine solo sulla prima parte.

use super::ids::{external_id, reply_message_id};
use super::pacer::Pacer;
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

fn parse_chat(chat: &str) -> Result<i64, ConnectorError> {
    chat.parse().map_err(|_| ConnectorError::Fatal("chat non valida".to_string()))
}

/// Manda il testo e ritorna l'id esterno di ogni parte, in ordine. Vuoto per un testo vuoto.
pub async fn send(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    out: &OutboundMessage,
) -> Result<Vec<String>, ConnectorError> {
    let chat_id = parse_chat(&out.chat)?;
    let mut reply_to = reply_message_id(&out.chat, out.reply_to_external.as_deref());
    let mut sent = Vec::new();
    for part in text::split(&out.text) {
        let wait = reserve_slot(pacer, &out.chat, ctx.now());
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        let message_id = api::send_message(ctx, chat_id, &part, reply_to.take()).await?;
        sent.push(external_id(chat_id, message_id));
    }
    Ok(sent)
}

#[cfg(test)]
mod tests;
