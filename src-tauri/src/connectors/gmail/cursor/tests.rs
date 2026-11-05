use super::*;

const NOW: i64 = 1_700_000_000_000;

#[test]
fn a_new_route_starts_now() {
    let mut cursor = Cursor::default();
    let route = cursor.route_mut("rte-1", NOW);
    assert_eq!(route.after_secs(), NOW / 1000);
    assert!(route.accepts(NOW));
    assert!(!route.accepts(NOW - 1));
    assert!(!route.is_recent("m1"));
}

#[test]
fn an_existing_route_is_not_reset() {
    let mut cursor = Cursor::default();
    cursor.route_mut("rte-1", NOW).advance(NOW + 5000);
    let route = cursor.route_mut("rte-1", NOW + 999_999);
    assert!(route.accepts(NOW));
    assert_eq!(route.last_ms, NOW + 5000);
}

#[test]
fn after_secs_goes_back_by_the_margin_but_never_before_the_floor() {
    let mut cursor = Cursor::default();
    let route = cursor.route_mut("r", NOW);
    assert_eq!(route.after_secs(), NOW / 1000);
    route.advance(NOW + MARGIN_MS / 2);
    assert_eq!(route.after_secs(), NOW / 1000);
    route.advance(NOW + 3 * MARGIN_MS);
    assert_eq!(route.after_secs(), (NOW + 2 * MARGIN_MS) / 1000);
}

#[test]
fn after_secs_floors_toward_the_past() {
    let route = RouteCursor { floor_ms: 0, last_ms: 1999 + MARGIN_MS, recent: vec![] };
    assert_eq!(route.after_secs(), 1);
    let negative = RouteCursor { floor_ms: -1500, last_ms: -1500, recent: vec![] };
    assert_eq!(negative.after_secs(), -2);
}

#[test]
fn advance_never_goes_back() {
    let mut route = RouteCursor::starting_at(NOW);
    route.advance(NOW + 100);
    route.advance(NOW - 100);
    assert_eq!(route.last_ms, NOW + 100);
}

#[test]
fn remember_avoids_duplicates() {
    let mut route = RouteCursor::starting_at(NOW);
    route.remember("m1", NOW + 1);
    route.remember("m1", NOW + 1);
    route.remember("m2", NOW + 2);
    assert!(route.is_recent("m1") && route.is_recent("m2"));
    assert_eq!(route.recent.len(), 2);
}

#[test]
fn old_ids_leave_the_memory_when_the_window_moves() {
    let mut route = RouteCursor::starting_at(NOW);
    route.remember("old", NOW + 1);
    route.remember("new", NOW + 2 * MARGIN_MS);
    route.advance(NOW + 2 * MARGIN_MS);
    assert!(!route.is_recent("old"));
    assert!(route.is_recent("new"));
}

#[test]
fn the_memory_is_capped_dropping_the_oldest() {
    let mut route = RouteCursor::starting_at(NOW);
    for n in 0..(RECENT_MAX + 50) {
        route.remember(&format!("m{n}"), NOW + i64::try_from(n).unwrap());
    }
    assert_eq!(route.recent.len(), RECENT_MAX);
    assert!(!route.is_recent("m0"));
    assert!(route.is_recent(&format!("m{}", RECENT_MAX + 49)));
}

#[test]
fn the_cursor_round_trips_through_json() {
    let mut cursor = Cursor::default();
    let route = cursor.route_mut("rte-a", NOW);
    route.remember("m1", NOW + 10);
    route.advance(NOW + 10);
    cursor.route_mut("rte-b", NOW + 1);
    let value = cursor.to_value();
    assert_eq!(value["routes"]["rte-a"]["recent"][0][0], "m1");
    assert_eq!(Cursor::parse(Some(&value)), cursor);
}

#[test]
fn a_missing_or_malformed_cursor_is_empty() {
    use serde_json::json;
    for value in [
        None,
        Some(json!(null)),
        Some(json!([])),
        Some(json!("x")),
        Some(json!({})),
        Some(json!({"routes": 5})),
        Some(json!({"routes": []})),
        Some(json!({"offset": 12})),
    ] {
        assert_eq!(Cursor::parse(value.as_ref()), Cursor::default(), "{value:?}");
    }
}

#[test]
fn malformed_routes_are_dropped_and_good_ones_kept() {
    use serde_json::json;
    let value = json!({"routes": {
        "good": {"floor_ms": 1, "last_ms": 2, "recent": [["m1", 2], [], ["", 3], ["m2"], ["m3", "x"], 5, ["m4", 2]]},
        "no-floor": {"last_ms": 2},
        "text": {"floor_ms": "1", "last_ms": 2},
        "null": null,
        "": {"floor_ms": 1, "last_ms": 2},
    }});
    let mut cursor = Cursor::parse(Some(&value));
    assert_eq!(cursor.routes.len(), 1);
    let good = cursor.route_mut("good", NOW);
    assert_eq!((good.floor_ms, good.last_ms), (1, 2));
    assert!(good.is_recent("m1") && good.is_recent("m4"));
    assert_eq!(good.recent.len(), 2);
    let restarted = cursor.route_mut("no-floor", NOW);
    assert_eq!(restarted.floor_ms, NOW);
}

#[test]
fn a_huge_recent_list_is_capped_on_parse() {
    use serde_json::json;
    let recent: Vec<Value> = (0..5000).map(|n| json!([format!("m{n}"), 100])).collect();
    let value = json!({"routes": {"r": {"floor_ms": 0, "last_ms": 100, "recent": recent}}});
    let mut cursor = Cursor::parse(Some(&value));
    assert_eq!(cursor.route_mut("r", 0).recent.len(), RECENT_MAX);
}

#[test]
fn too_many_routes_are_capped_on_parse() {
    use serde_json::json;
    let mut routes = serde_json::Map::new();
    for n in 0..(MAX_ROUTES_PER_INSTANCE + 20) {
        routes.insert(format!("r{n}"), json!({"floor_ms": 0, "last_ms": 0}));
    }
    let cursor = Cursor::parse(Some(&json!({ "routes": routes })));
    assert_eq!(cursor.routes.len(), MAX_ROUTES_PER_INSTANCE);
}

#[test]
fn retain_keeps_only_live_routes() {
    let mut cursor = Cursor::default();
    cursor.route_mut("a", NOW);
    cursor.route_mut("b", NOW);
    cursor.retain(&["b".to_string(), "zzz".to_string()]);
    assert_eq!(cursor.routes.keys().collect::<Vec<_>>(), vec!["b"]);
    cursor.retain(&[]);
    assert!(cursor.routes.is_empty());
}

#[test]
fn extreme_timestamps_do_not_overflow() {
    let mut route = RouteCursor { floor_ms: i64::MIN, last_ms: i64::MIN, recent: vec![] };
    let _ = route.after_secs();
    route.advance(i64::MAX);
    let _ = route.after_secs();
    route.remember("x", i64::MIN);
    assert!(route.accepts(i64::MAX));
}

#[test]
fn a_route_exists_only_after_route_mut() {
    let mut cursor = Cursor::default();
    assert!(!cursor.has_route("rte-1"));
    cursor.route_mut("rte-1", NOW);
    assert!(cursor.has_route("rte-1"));
    assert!(Cursor::parse(Some(&cursor.to_value())).has_route("rte-1"));
}
