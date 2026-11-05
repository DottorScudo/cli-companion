use super::*;
use crate::connectors::gmail::cursor::Cursor;
use crate::connectors::gmail::fixtures::{parsed, THREAD};
use crate::hub::attachments::IncomingFile;

const NOW: i64 = 1_700_000_000_000;

#[test]
fn the_item_carries_the_message() {
    let message = parsed("m1", NOW);
    let item = build_item("rte-1", &message, &Files::default());
    assert_eq!(item.route, "rte-1");
    assert_eq!(item.external_id, "m1");
    assert_eq!(item.conversation_key.as_deref(), Some(THREAD));
    assert_eq!(item.title, "Riunione");
    assert_eq!(item.sender, "Mario Rossi <mario@gmail.com>");
    assert!(item.body.starts_with("Oggetto: Riunione\nDa: Mario Rossi <mario@gmail.com>\nData: "), "{}", item.body);
    assert!(item.body.ends_with("\n\nCiao\nmondo"), "{}", item.body);
    assert!(item.files.is_empty());
}

#[test]
fn files_and_the_not_imported_line_end_up_in_the_item() {
    let files = Files {
        files: vec![IncomingFile { name: "a.pdf".to_string(), bytes: vec![1, 2, 3] }],
        not_imported: vec![("grande.zip".to_string(), 40 * 1024 * 1024)],
    };
    let item = build_item("rte-1", &parsed("m1", NOW), &files);
    assert_eq!(item.files, files.files);
    assert!(item.body.contains("Allegati non importati: grande.zip (40 MB)"), "{}", item.body);
}

#[test]
fn two_routes_make_two_items_from_one_message() {
    let message = parsed("m1", NOW);
    let a = build_item("rte-a", &message, &Files::default());
    let b = build_item("rte-b", &message, &Files::default());
    assert_ne!(a.route, b.route);
    assert_eq!((a.external_id, a.body), (b.external_id, b.body));
}

#[test]
fn delivering_remembers_the_message_and_moves_the_start_forward() {
    let mut cursor = Cursor::default();
    let rc = cursor.route_mut("rte-1", NOW);
    let before = rc.after_secs();
    deliver(rc, "rte-1", &parsed("m1", NOW + 3_600_000), &Files::default());
    assert!(rc.is_recent("m1"));
    assert!(rc.after_secs() > before);
}
