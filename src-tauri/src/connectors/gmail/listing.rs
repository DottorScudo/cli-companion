//! Gli id dei messaggi nuovi di ogni regola: `messages.list` con la ricerca dell'utente
//! limitata a dopo l'ultimo messaggio visto (meno il margine del cursore).

use super::api::{get_json, into_error, NORMAL_RESPONSE_MAX};
use super::api_urls::{list_url, search_query};
use super::classify::ApiFailure;
use super::cursor::{Cursor, RouteCursor};
use super::keys::QUERY_KEY;
use super::list::parse_list;
use super::tokens::TokenCache;
use super::validate::validate_query;
use crate::connectors::contracts::{ActiveRoute, ConnectorContext, ConnectorError};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;

/// Pagine di risultati lette al massimo per regola e per giro (100 messaggi l'una).
pub const LIST_PAGES_MAX: usize = 5;

/// Gli id non ancora visti e senza doppioni, dal piu' vecchio (Gmail li da' dal piu' recente).
pub fn fresh_oldest_first(ids: Vec<String>, rc: &RouteCursor) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut fresh: Vec<String> = ids.into_iter().filter(|id| !rc.is_recent(id) && seen.insert(id.clone())).collect();
    fresh.reverse();
    fresh
}

/// Gli id delle pagine di risultati; `None` se Gmail rifiuta la ricerca (400).
async fn list_pages(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    search: &str,
) -> Result<Option<Vec<String>>, ConnectorError> {
    let mut ids = Vec::new();
    let mut page_token: Option<String> = None;
    for _ in 0..LIST_PAGES_MAX {
        let url = list_url(search, page_token.as_deref());
        let body = match get_json(cache, ctx, &url, NORMAL_RESPONSE_MAX).await {
            Ok(body) => body,
            Err(ApiFailure::BadRequest) => return Ok(None),
            Err(other) => return Err(into_error(other)),
        };
        let page = parse_list(&body)?;
        ids.extend(page.ids);
        page_token = page.next_page_token;
        if page_token.is_none() {
            break;
        }
    }
    Ok(Some(ids))
}

/// La ricerca valida di una regola, se il filtro ce l'ha.
fn route_query(route: &ActiveRoute) -> Option<&str> {
    let query = route.filter.get(QUERY_KEY).and_then(Value::as_str)?;
    validate_query(query).ok().map(|_| query)
}

/// Per ogni regola, gli id da leggere (dal piu' vecchio). Una regola appena nata non elenca nulla
/// in questo giro: nessuna email precedente alla regola si importa. Una regola con una ricerca non
/// valida, o che Gmail rifiuta, salta il giro (nel log solo l'id della regola).
pub async fn list_routes(
    cache: &Mutex<TokenCache>,
    ctx: &ConnectorContext,
    routes: &[ActiveRoute],
    cursor: &mut Cursor,
    now_ms: i64,
) -> Result<Vec<(String, Vec<String>)>, ConnectorError> {
    let mut lists = Vec::new();
    for route in routes {
        let existed = cursor.has_route(&route.id);
        let rc = cursor.route_mut(&route.id, now_ms);
        if !existed {
            continue;
        }
        let Some(query) = route_query(route) else {
            log::warn!("gmail {}: regola {} con ricerca non valida, saltata", ctx.instance(), route.id);
            continue;
        };
        match list_pages(cache, ctx, &search_query(query, rc.after_secs())).await? {
            Some(ids) => lists.push((route.id.clone(), fresh_oldest_first(ids, cursor.route_mut(&route.id, now_ms)))),
            None => log::warn!("gmail {}: ricerca rifiutata da Gmail per la regola {}", ctx.instance(), route.id),
        }
    }
    Ok(lists)
}

#[cfg(test)]
mod tests;
