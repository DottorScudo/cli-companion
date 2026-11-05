//! Un giro di `poll`: gli id nuovi di ogni regola, i messaggi da leggere (a turno fra le
//! regole, al piu' `ITEMS_PER_ROUND`) e gli elementi con il cursore nuovo. Il runtime salva il
//! cursore solo dopo la consegna: gli elementi non superano mai `ITEMS_PER_ROUND`, altrimenti
//! il cursore non verrebbe salvato. Nei log solo l'id dell'istanza e i conteggi.

use super::cursor::Cursor;
use super::gather::{Gather, Step};
use super::listing::list_routes;
use super::plan::plan_fetch;
use super::tokens::TokenCache;
use crate::connectors::contracts::{ActiveRoute, ConnectorContext, ConnectorError, PollBatch, SourceItem};
use crate::connectors::limits::ITEMS_PER_ROUND;
use serde_json::Value;
use std::sync::Mutex;

/// Legge i messaggi pianificati; il secondo valore dice se un budget ha fermato il giro.
async fn read_plan(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    cursor: &mut Cursor,
    plan: &[(String, String)],
    started_ms: i64,
) -> Result<(Vec<SourceItem>, bool), ConnectorError> {
    let mut gather = Gather::new(cache, ctx, started_ms);
    let mut items = Vec::new();
    for (index, (route, id)) in plan.iter().enumerate() {
        let wanted_again = plan.iter().skip(index + 1).any(|(_, other)| other == id);
        match gather.step(cursor, route, id, wanted_again).await? {
            Step::Item(item) => items.push(*item),
            Step::Skipped => {}
            Step::Stop => return Ok((items, true)),
        }
    }
    Ok((items, false))
}

pub async fn poll(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    routes: &[ActiveRoute],
    cursor: Option<Value>,
) -> Result<PollBatch, ConnectorError> {
    let now = ctx.now();
    let mut cursor = Cursor::parse(cursor.as_ref());
    let live: Vec<String> = routes.iter().map(|r| r.id.clone()).collect();
    cursor.retain(&live);
    let lists = list_routes(cache, ctx, routes, &mut cursor, now).await?;
    let (plan, more_planned) = plan_fetch(&lists, ITEMS_PER_ROUND);
    let (items, stopped) = read_plan(cache, ctx, &mut cursor, &plan, now).await?;
    if !items.is_empty() {
        log::info!("gmail {}: {} email lette", ctx.instance(), items.len());
    }
    Ok(PollBatch { items, cursor: cursor.to_value(), more: more_planned || stopped })
}

#[cfg(test)]
mod tests;
