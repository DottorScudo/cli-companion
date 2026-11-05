use super::*;
use crate::connectors::limits::LONG_POLL_MAX;

#[test]
fn the_first_poll_asks_only_for_the_latest_update_without_waiting() {
    assert_eq!(latest_update_body(), json!({"offset": -1, "timeout": 0}));
}

#[test]
fn the_poll_waits_less_than_the_runtime_allows() {
    let body = poll_body(42);
    assert_eq!(body["offset"], 42);
    assert_eq!(body["timeout"], 50);
    assert_eq!(body["limit"], 100);
    assert_eq!(body["allowed_updates"], json!(["message", "my_chat_member"]));
    assert!(POLL_WAIT_SECS <= LONG_POLL_MAX.as_secs());
}

#[test]
fn deleting_the_webhook_keeps_the_pending_updates() {
    assert_eq!(delete_webhook_body(), json!({"drop_pending_updates": false}));
}

#[test]
fn a_message_is_plain_text_without_link_previews() {
    let body = send_message_body(-5, "ciao", None);
    assert_eq!(body, json!({"chat_id": -5, "text": "ciao", "link_preview_options": {"is_disabled": true}}));
    assert!(body.get("parse_mode").is_none() && body.get("reply_parameters").is_none());
}

#[test]
fn a_reply_is_linked_even_if_the_original_is_gone() {
    let body = send_message_body(5, "ok", Some(9));
    assert_eq!(body["reply_parameters"], json!({"message_id": 9, "allow_sending_without_reply": true}));
}

#[test]
fn small_bodies() {
    assert_eq!(leave_chat_body(-7), json!({"chat_id": -7}));
    assert_eq!(get_file_body("AgAD"), json!({"file_id": "AgAD"}));
}
