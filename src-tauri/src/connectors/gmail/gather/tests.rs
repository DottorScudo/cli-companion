//! Le decisioni di un passo, senza rete: i messaggi sono gia' nella cache del giro oppure non
//! hanno allegati da scaricare.

use super::*;
use crate::connectors::gmail::budget::ROUND_TIME_MAX_MS;
use crate::connectors::gmail::descriptor::DESCRIPTOR;
use crate::connectors::gmail::fixtures::parsed;
use crate::connectors::testing::{test_context, TestContext};
use serde_json::json;

const NOW: i64 = 1_700_000_000_000;

fn setup() -> (TestContext, Mutex<TokenCache>, Cursor) {
    let t = test_context(&DESCRIPTOR, json!({}));
    let mut cursor = Cursor::default();
    cursor.route_mut("rte-a", t.ctx.now());
    cursor.route_mut("rte-b", t.ctx.now());
    (t, Mutex::new(TokenCache::new()), cursor)
}

fn step(gather: &mut Gather<'_>, cursor: &mut Cursor, route: &str, id: &str, keep: bool) -> Step {
    tauri::async_runtime::block_on(gather.step(cursor, route, id, keep)).unwrap()
}

fn cached(gather: &mut Gather<'_>, id: &str, internal_ms: i64) {
    gather.messages.insert(id.to_string(), Some(Arc::new(parsed(id, internal_ms))));
}

#[test]
fn a_new_message_becomes_an_item_and_is_remembered() {
    let (t, cache, mut cursor) = setup();
    let mut gather = Gather::new(&cache, &t.ctx, NOW);
    cached(&mut gather, "m1", NOW + 5000);
    let Step::Item(item) = step(&mut gather, &mut cursor, "rte-a", "m1", false) else { panic!("atteso un elemento") };
    assert_eq!((item.route.as_str(), item.external_id.as_str()), ("rte-a", "m1"));
    assert!(cursor.route_mut("rte-a", NOW).is_recent("m1"));
}

#[test]
fn a_message_older_than_the_rule_is_remembered_but_not_delivered() {
    let (t, cache, mut cursor) = setup();
    let mut gather = Gather::new(&cache, &t.ctx, NOW);
    cached(&mut gather, "old", NOW - 1);
    assert!(matches!(step(&mut gather, &mut cursor, "rte-a", "old", false), Step::Skipped));
    assert!(cursor.route_mut("rte-a", NOW).is_recent("old"));
}

#[test]
fn a_vanished_or_unreadable_message_is_skipped_and_remembered() {
    let (t, cache, mut cursor) = setup();
    let mut gather = Gather::new(&cache, &t.ctx, NOW);
    gather.messages.insert("gone".to_string(), None);
    assert!(matches!(step(&mut gather, &mut cursor, "rte-a", "gone", false), Step::Skipped));
    assert!(cursor.route_mut("rte-a", NOW).is_recent("gone"));
}

#[test]
fn two_routes_sharing_a_message_get_one_item_each() {
    let (t, cache, mut cursor) = setup();
    let mut gather = Gather::new(&cache, &t.ctx, NOW);
    cached(&mut gather, "m1", NOW + 5000);
    let first = step(&mut gather, &mut cursor, "rte-a", "m1", true);
    let second = step(&mut gather, &mut cursor, "rte-b", "m1", false);
    let routes: Vec<_> = [first, second]
        .into_iter()
        .map(|s| match s {
            Step::Item(item) => item.route,
            other => panic!("atteso un elemento, trovato {other:?}"),
        })
        .collect();
    assert_eq!(routes, vec!["rte-a".to_string(), "rte-b".to_string()]);
}

#[test]
fn the_time_budget_stops_the_round_after_the_first_message() {
    let (t, cache, mut cursor) = setup();
    let mut gather = Gather::new(&cache, &t.ctx, t.ctx.now());
    cached(&mut gather, "m1", NOW + 5000);
    cached(&mut gather, "m2", NOW + 6000);
    t.clock.advance(ROUND_TIME_MAX_MS * 2);
    assert!(matches!(step(&mut gather, &mut cursor, "rte-a", "m1", false), Step::Item(_)));
    assert!(matches!(step(&mut gather, &mut cursor, "rte-a", "m2", false), Step::Stop));
    assert!(!cursor.route_mut("rte-a", NOW).is_recent("m2"));
}
