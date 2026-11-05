use super::*;
use crate::connectors::gmail::attachments::Files;
use crate::connectors::gmail::fixtures::parsed;
use crate::connectors::gmail::items::deliver;
use serde_json::json;

const NOW: i64 = 1_700_000_000_000;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn ids_are_reversed_to_oldest_first_without_duplicates() {
    let mut cursor = Cursor::default();
    let rc = cursor.route_mut("r", NOW);
    assert_eq!(fresh_oldest_first(ids(&["c", "b", "a", "b"]), rc), ids(&["a", "b", "c"]));
}

#[test]
fn recent_ids_are_dropped() {
    let mut cursor = Cursor::default();
    let rc = cursor.route_mut("r", NOW);
    deliver(rc, "r", &parsed("b", NOW + 1000), &Files::default());
    assert_eq!(fresh_oldest_first(ids(&["c", "b", "a"]), rc), ids(&["a", "c"]));
}

#[test]
fn a_route_needs_a_valid_query() {
    let route = |filter| ActiveRoute { id: "r".to_string(), filter };
    assert_eq!(route_query(&route(json!({"query": "from:a@b.it"}))), Some("from:a@b.it"));
    for filter in [json!({}), json!({"query": "(x"}), json!({"query": ""})] {
        assert_eq!(route_query(&route(filter)), None);
    }
}

#[test]
fn the_page_limit_is_small() {
    const { assert!(LIST_PAGES_MAX >= 1 && LIST_PAGES_MAX <= 10) };
}
