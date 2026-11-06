//! L'abbinamento da un supergruppo forum. Prima che il runtime veda il codice (e lo
//! consumi), il connettore controlla con `getChatMember` che il bot sia amministratore
//! del gruppo con il permesso di gestire i topic: senza, risponde con le istruzioni e
//! il codice resta valido. Le decisioni e i testi sono puri.

use super::api;
use super::forum::is_group;
use super::pacer::Pacer;
use super::send::{parse_chat, send};
use crate::connectors::contracts::{ConnectorContext, ConnectorError, InboundMessage, OutboundMessage};
use serde_json::Value;
use std::sync::Mutex;

const STATUS_ADMINISTRATOR: &str = "administrator";

/// Il bot non ha i permessi che servono: cosa fare, passo per passo.
pub const MISSING_RIGHTS_TEXT: &str = "Per usare questo gruppo il bot deve esserne amministratore con il permesso \
    «Gestire i topic». In Telegram apri le informazioni del gruppo › Amministratori, aggiungi il bot (o modificalo) \
    e attiva «Gestire i topic». Poi invia di nuovo «/start» con il codice: non è stato usato (se è scaduto, \
    generane uno nuovo dall'app).";
/// I permessi non si sono potuti controllare (errore di Telegram o della rete).
pub const UNVERIFIED_TEXT: &str = "Non riesco a controllare i permessi del bot in questo gruppo: invia di nuovo \
    «/start» con il codice tra qualche istante. Il codice non è stato usato.";

/// Cosa fare di un invito di abbinamento da un gruppo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Il bot puo' gestire i topic: il codice passa al runtime.
    Pass,
    /// Il codice non passa; il bot risponde con questo testo.
    Refuse(&'static str),
}

/// L'id del bot dalla risposta di `getMe`.
pub fn bot_id(me: &Value) -> Option<i64> {
    me.get("id")?.as_i64().filter(|id| *id > 0)
}

/// Dalla risposta di `getChatMember`: il bot e' amministratore e puo' gestire i topic?
pub fn can_manage_topics(member: &Value) -> bool {
    member.get("status").and_then(Value::as_str) == Some(STATUS_ADMINISTRATOR)
        && member.get("can_manage_topics").and_then(Value::as_bool) == Some(true)
}

/// La decisione dall'esito del controllo dei permessi. Gli errori che fermano il
/// supervisore (il token) risalgono; gli altri non consumano il codice.
pub fn admission(rights: Result<bool, ConnectorError>) -> Result<Admission, ConnectorError> {
    match rights {
        Ok(true) => Ok(Admission::Pass),
        Ok(false) => Ok(Admission::Refuse(MISSING_RIGHTS_TEXT)),
        Err(e) if e.stops_the_supervisor() => Err(e),
        Err(_) => Ok(Admission::Refuse(UNVERIFIED_TEXT)),
    }
}

/// Il bot puo' gestire i topic nel gruppo `chat`?
async fn bot_rights(ctx: &ConnectorContext, chat: &str) -> Result<bool, ConnectorError> {
    let chat_id = parse_chat(chat)?;
    let me = api::get_me(ctx).await?;
    let bot =
        bot_id(&me).ok_or_else(|| ConnectorError::Transient("risposta di Telegram non riconosciuta".to_string()))?;
    Ok(can_manage_topics(&api::get_chat_member(ctx, chat_id, bot).await?))
}

/// La risposta a un invito rifiutato, nel topic in cui e' stato scritto. Un errore che
/// non ferma il supervisore resta nel log: l'utente puo' riprovare.
async fn refuse(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    message: &InboundMessage,
    text: &str,
) -> Result<(), ConnectorError> {
    let out = OutboundMessage {
        chat: message.chat.clone(),
        subchat: message.subchat.clone(),
        text: text.to_string(),
        reply_to_external: None,
    };
    match send(pacer, ctx, &out).await {
        Ok(_) => Ok(()),
        Err(e) if e.stops_the_supervisor() => Err(e),
        Err(e) => {
            log::warn!("connettori {}: telegram: istruzioni per il gruppo non inviate ({})", ctx.instance(), e.class());
            Ok(())
        }
    }
}

/// Toglie dal lotto gli inviti di abbinamento da un gruppo in cui il bot non puo'
/// gestire i topic (dopo aver risposto con le istruzioni); il resto passa intatto.
pub async fn screen(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    messages: Vec<InboundMessage>,
) -> Result<Vec<InboundMessage>, ConnectorError> {
    let mut admitted = Vec::with_capacity(messages.len());
    for message in messages {
        if message.pairing_code.is_none() || !is_group(&message.chat) {
            admitted.push(message);
            continue;
        }
        match admission(bot_rights(ctx, &message.chat).await)? {
            Admission::Pass => admitted.push(message),
            Admission::Refuse(text) => {
                log::info!("connettori {}: telegram: abbinamento da un gruppo senza i permessi", ctx.instance());
                refuse(pacer, ctx, &message, text).await?;
            }
        }
    }
    Ok(admitted)
}

#[cfg(test)]
mod tests;
