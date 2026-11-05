//! Le azioni dell'interfaccia: `Autorizza` (consenso OAuth) e `Prova` (l'account raggiungibile).

use super::api::fetch_profile;
use super::authorize::authorize;
use super::profile::test_outcome;
use super::tokens::TokenCache;
use crate::connectors::contracts::{ActionOutcome, ConnectorContext, ConnectorError};
use crate::connectors::descriptor::ActionId;
use std::sync::Mutex;

async fn test(cache: &Mutex<TokenCache>, ctx: &ConnectorContext) -> Result<ActionOutcome, ConnectorError> {
    Ok(test_outcome(&fetch_profile(cache, ctx).await?))
}

pub async fn run(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    action: ActionId,
) -> Result<ActionOutcome, ConnectorError> {
    match action {
        ActionId::Authorize => authorize(cache, ctx).await,
        ActionId::Test => test(cache, ctx).await,
        ActionId::Pair => Err(ConnectorError::Fatal("azione non dichiarata".to_string())),
    }
}
