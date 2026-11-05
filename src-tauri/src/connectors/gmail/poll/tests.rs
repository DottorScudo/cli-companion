//! `poll` senza rete: nessuna regola, regole appena nate, ricerche non valide.

use super::*;
use crate::connectors::gmail::descriptor::DESCRIPTOR;
use crate::connectors::testing::{test_context, TestContext};
use serde_json::json;

fn context() -> TestContext {
    test_context(&DESCRIPTOR, json!({}))
}

fn route(id: &str, query: &str) -> ActiveRoute {
    ActiveRoute { id: id.to_string(), filter: json!({"query": query}) }
}

fn run(t: &TestContext, routes: &[ActiveRoute], cursor: Option<Value>) -> PollBatch {
    let cache = Mutex::new(TokenCache::new());
    tauri::async_runtime::block_on(poll(&cache, &t.ctx, routes, cursor)).unwrap()
}

#[test]
fn without_routes_there_is_nothing_to_do() {
    let batch = run(&context(), &[], None);
    assert!(batch.items.is_empty() && !batch.more);
    assert_eq!(batch.cursor, json!({"routes": {}}));
}

#[test]
fn a_new_route_starts_now_and_imports_nothing_yet() {
    let t = context();
    let batch = run(&t, &[route("rte-1", "from:mario@gmail.com")], None);
    assert!(batch.items.is_empty() && !batch.more);
    let floor = batch.cursor["routes"]["rte-1"]["floor_ms"].as_i64();
    assert_eq!(floor, Some(t.ctx.now()));
}

#[test]
fn routes_that_no_longer_exist_leave_the_cursor() {
    let t = context();
    let first = run(&t, &[route("rte-1", "a"), route("rte-2", "b")], None).cursor;
    let second = run(&t, &[route("rte-2", "b2(")], Some(first)).cursor;
    assert!(second["routes"].get("rte-1").is_none());
    assert!(second["routes"].get("rte-2").is_some());
}

#[test]
fn an_invalid_query_skips_the_route_without_failing_the_round() {
    let t = context();
    let seed = run(&t, &[route("rte-1", "a")], None).cursor;
    let batch = run(&t, &[route("rte-1", "(non chiusa")], Some(seed.clone()));
    assert!(batch.items.is_empty());
    assert_eq!(batch.cursor, seed);
}

#[test]
fn a_round_never_plans_more_items_than_the_runtime_delivers() {
    let ids = |prefix: &str| (0..100).map(|i| format!("{prefix}{i}")).collect::<Vec<_>>();
    let lists = vec![("a".to_string(), ids("a")), ("b".to_string(), ids("b"))];
    let (plan, more) = plan_fetch(&lists, ITEMS_PER_ROUND);
    assert_eq!(plan.len(), ITEMS_PER_ROUND);
    assert!(more);
}
