//! Le azioni dell'interfaccia: `Prova` (chi e' il bot) e `Abbina` (il link di invito e le
//! istruzioni per la chat privata e per un gruppo con i Topic).
//! Gli esiti si costruiscono con funzioni pure; l'unica rete e' `getMe`.

use super::api;
use super::commands::valid_payload;
use super::names::{valid_username, BOT_NAME_MAX};
use crate::connectors::contracts::{ActionOutcome, ConnectorContext, ConnectorError};
use crate::connectors::descriptor::ActionId;
use crate::connectors::text::clean_display;
use serde_json::{json, Value};

/// La chiave dello stato privato con lo username del bot.
pub const BOT_USERNAME_KEY: &str = "bot_username";
/// Minuti di validita' del codice di abbinamento (li decide il runtime).
const PAIRING_MINUTES: u32 = 10;
/// Il permesso che il link `?startgroup=` chiede di dare al bot nel gruppo scelto.
const GROUP_ADMIN_RIGHTS: &str = "manage_topics";

/// Chi e' il bot, da `getMe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotInfo {
    pub username: String,
    pub first_name: String,
}

pub fn parse_bot(result: &Value) -> Result<BotInfo, ConnectorError> {
    let username = result.get("username").and_then(Value::as_str).filter(|u| valid_username(u));
    let first_name = result.get("first_name").and_then(Value::as_str).map(|n| clean_display(n, BOT_NAME_MAX));
    match (username, first_name) {
        (Some(username), Some(first_name)) => Ok(BotInfo { username: username.to_string(), first_name }),
        _ => Err(ConnectorError::Transient("risposta di Telegram non riconosciuta".to_string())),
    }
}

pub fn test_outcome(bot: &BotInfo) -> ActionOutcome {
    ActionOutcome {
        message: format!("Bot raggiungibile: @{} ({}).", bot.username, bot.first_name),
        detail: Some(format!("@{}", bot.username)),
    }
}

/// Il codice finisce in un URL: deve avere la forma di un codice di invito.
pub fn pair_outcome(bot: &BotInfo, code: &str) -> Result<ActionOutcome, ConnectorError> {
    if !valid_payload(code) {
        return Err(ConnectorError::Fatal("codice di abbinamento non valido".to_string()));
    }
    let bot_name = &bot.username;
    Ok(ActionOutcome {
        message: format!(
            "Chat privata: apri il link da Telegram, oppure invia «/start {code}» a @{bot_name}. Gruppo con i Topic \
             (ogni thread nel suo topic): crea il gruppo, attiva i Topic, aggiungi @{bot_name} come amministratore con \
             il permesso «Gestire i topic» (puoi farlo dal link \
             https://t.me/{bot_name}?startgroup={code}&admin={GROUP_ADMIN_RIGHTS}) e scrivi «/start {code}» nel \
             gruppo. Il codice vale {PAIRING_MINUTES} minuti."
        ),
        detail: Some(format!("https://t.me/{bot_name}?start={code}")),
    })
}

async fn bot_info(ctx: &ConnectorContext) -> Result<BotInfo, ConnectorError> {
    parse_bot(&api::get_me(ctx).await?)
}

async fn test(ctx: &ConnectorContext) -> Result<ActionOutcome, ConnectorError> {
    let bot = bot_info(ctx).await?;
    ctx.state_set(BOT_USERNAME_KEY, json!(bot.username))?;
    Ok(test_outcome(&bot))
}

async fn pair(ctx: &ConnectorContext) -> Result<ActionOutcome, ConnectorError> {
    let code = ctx
        .pairing_code()
        .ok_or_else(|| ConnectorError::Fatal("codice di abbinamento mancante".to_string()))?
        .to_string();
    pair_outcome(&bot_info(ctx).await?, &code)
}

pub async fn run(ctx: &ConnectorContext, action: ActionId) -> Result<ActionOutcome, ConnectorError> {
    match action {
        ActionId::Test => test(ctx).await,
        ActionId::Pair => pair(ctx).await,
        ActionId::Authorize => Err(ConnectorError::Fatal("azione non dichiarata".to_string())),
    }
}

#[cfg(test)]
mod tests;
