use super::*;
use serde_json::{json, Value};

fn thread(id: &str) -> ThreadKey {
    ThreadKey { channel: Channel::General, thread_id: id.to_string() }
}

fn place(chat: &str, subchat: &str) -> SubchatKey {
    SubchatKey { chat: chat.to_string(), subchat: subchat.to_string() }
}

#[test]
fn a_mapping_is_found_in_both_directions() {
    let mut map = SubchatMap::default();
    assert!(map.insert(thread("thr-1"), place("g1", "7")));
    assert_eq!(map.subchat_of(&thread("thr-1"), "g1").as_deref(), Some("7"));
    assert_eq!(map.thread_of(&place("g1", "7")), Some(thread("thr-1")));
    assert_eq!(map.subchat_of(&thread("thr-1"), "g2"), None, "un'altra chat non ha la sotto-chat");
    assert_eq!(map.thread_of(&place("g2", "7")), None);
}

#[test]
fn the_same_thread_can_have_a_subchat_in_every_chat() {
    let mut map = SubchatMap::default();
    map.insert(thread("thr-1"), place("g1", "7"));
    map.insert(thread("thr-1"), place("g2", "9"));
    assert_eq!(map.len(), 2);
    assert_eq!(map.subchat_of(&thread("thr-1"), "g1").as_deref(), Some("7"));
    assert_eq!(map.subchat_of(&thread("thr-1"), "g2").as_deref(), Some("9"));
}

#[test]
fn remapping_drops_the_stale_entries_of_both_sides() {
    let mut map = SubchatMap::default();
    map.insert(thread("thr-1"), place("g1", "7"));
    // Il thread ha una sotto-chat nuova: la vecchia non porta piu' li'.
    map.insert(thread("thr-1"), place("g1", "8"));
    assert_eq!(map.thread_of(&place("g1", "7")), None);
    // La sotto-chat passa a un altro thread: il primo non la ritrova.
    map.insert(thread("thr-2"), place("g1", "8"));
    assert_eq!(map.subchat_of(&thread("thr-1"), "g1"), None);
    assert_eq!(map.thread_of(&place("g1", "8")), Some(thread("thr-2")));
    assert_eq!(map.len(), 1);
}

#[test]
fn removing_a_subchat_clears_both_directions() {
    let mut map = SubchatMap::default();
    map.insert(thread("thr-1"), place("g1", "7"));
    assert_eq!(map.remove_subchat(&place("g1", "7")), Some(thread("thr-1")));
    assert_eq!((map.len(), map.subchat_of(&thread("thr-1"), "g1")), (0, None));
    assert_eq!(map.remove_subchat(&place("g1", "7")), None);
}

#[test]
fn the_map_is_bounded_and_drops_the_least_recently_used() {
    let mut map = SubchatMap::with_capacity(3);
    for i in 1..=3 {
        map.insert(thread(&format!("thr-{i}")), place("g1", &i.to_string()));
    }
    // Usare la prima la rende la piu' recente: esce la seconda.
    assert!(map.subchat_of(&thread("thr-1"), "g1").is_some());
    map.insert(thread("thr-4"), place("g1", "4"));
    assert_eq!(map.len(), 3);
    assert_eq!(map.subchat_of(&thread("thr-2"), "g1"), None);
    assert_eq!(map.thread_of(&place("g1", "2")), None);
    assert!(map.thread_of(&place("g1", "1")).is_some() && map.thread_of(&place("g1", "4")).is_some());
}

#[test]
fn the_default_capacity_is_the_runtime_limit() {
    let mut map = SubchatMap::default();
    for i in 0..SUBCHATS_MAX + 5 {
        map.insert(thread(&format!("thr-{i}")), place("g1", &i.to_string()));
    }
    assert_eq!(map.len(), SUBCHATS_MAX);
    assert_eq!(map.thread_of(&place("g1", "4")), None);
    assert!(map.thread_of(&place("g1", "5")).is_some());
}

#[test]
fn invalid_ids_are_refused() {
    let mut map = SubchatMap::default();
    let long = "1".repeat(SUBCHAT_ID_MAX + 1);
    for bad in ["", "7 8", "7\n", "ü", "a/b", long.as_str()] {
        assert!(!valid_subchat_id(bad), "{bad:?}");
        assert!(!map.insert(thread("thr-1"), place("g1", bad)), "{bad:?}");
    }
    assert!(!map.insert(thread("thr 1"), place("g1", "7")), "id di thread non valido");
    assert!(!map.insert(thread("thr-1"), place("", "7")), "chat vuota");
    assert_eq!(map.len(), 0);
    for good in ["7", "-100123", "topic_1.a:b", "1".repeat(SUBCHAT_ID_MAX).as_str()] {
        assert!(valid_subchat_id(good), "{good:?}");
    }
}

#[test]
fn the_map_roundtrips_through_json_keeping_the_order() {
    let mut map = SubchatMap::default();
    map.insert(thread("thr-1"), place("g1", "7"));
    map.insert(
        ThreadKey { channel: Channel::Workspace("ws-1".to_string()), thread_id: "thr-2".to_string() },
        place("g1", "8"),
    );
    let text = serde_json::to_value(&map).unwrap();
    assert_eq!(
        text,
        json!([
            {"channel": "general", "thread_id": "thr-1", "chat": "g1", "subchat": "7"},
            {"channel": "ws-1", "thread_id": "thr-2", "chat": "g1", "subchat": "8"}
        ])
    );
    let back: SubchatMap = serde_json::from_value(text).unwrap();
    assert_eq!(back, map);
}

#[test]
fn invalid_entries_in_a_file_are_dropped_and_the_newest_are_kept() {
    let mut entries: Vec<Value> = (0..SUBCHATS_MAX + 2)
        .map(|i| json!({"channel": "general", "thread_id": format!("thr-{i}"), "chat": "g1", "subchat": i.to_string()}))
        .collect();
    entries.push(json!({"channel": "general", "thread_id": "thr-x", "chat": "g1", "subchat": "a b"}));
    let mut map: SubchatMap = serde_json::from_value(Value::Array(entries)).unwrap();
    assert_eq!(map.len(), SUBCHATS_MAX);
    assert_eq!(map.thread_of(&place("g1", "1")), None);
    assert!(map.thread_of(&place("g1", "2")).is_some());
    assert_eq!(map.subchat_of(&thread("thr-x"), "g1"), None);
}
