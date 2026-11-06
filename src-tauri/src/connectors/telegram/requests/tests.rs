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
    let body = send_message_body(Destination { chat_id: -5, topic_id: None }, "ciao", None);
    assert_eq!(body, json!({"chat_id": -5, "text": "ciao", "link_preview_options": {"is_disabled": true}}));
    assert!(body.get("parse_mode").is_none() && body.get("reply_parameters").is_none());
}

#[test]
fn a_reply_is_linked_even_if_the_original_is_gone() {
    let body = send_message_body(Destination { chat_id: 5, topic_id: None }, "ok", Some(9));
    assert_eq!(body["reply_parameters"], json!({"message_id": 9, "allow_sending_without_reply": true}));
}

#[test]
fn small_bodies() {
    assert_eq!(leave_chat_body(-7), json!({"chat_id": -7}));
    assert_eq!(get_file_body("AgAD"), json!({"file_id": "AgAD"}));
}

const FORUM: i64 = -1_001_234_567_890;

#[test]
fn a_message_in_a_topic_names_the_topic() {
    let body = send_message_body(Destination { chat_id: FORUM, topic_id: Some(55) }, "ciao", Some(9));
    assert_eq!(body["chat_id"], FORUM);
    assert_eq!(body["message_thread_id"], 55);
    assert_eq!(body["reply_parameters"]["message_id"], 9);
    assert!(body.get("parse_mode").is_none());
    assert!(send_message_body(Destination { chat_id: FORUM, topic_id: None }, "x", None)
        .get("message_thread_id")
        .is_none());
}

#[test]
fn a_new_topic_has_its_name_cut_at_128_units_with_unicode_intact() {
    let title = format!("Generale › {}", "😀".repeat(100));
    let body = create_forum_topic_body(FORUM, &title);
    let name = body["name"].as_str().unwrap();
    assert!(name.starts_with("Generale › 😀"));
    assert!(name.encode_utf16().count() <= 128);
    assert!(name.chars().skip("Generale › ".chars().count()).all(|c| c == '😀'), "nessun carattere spezzato");
    assert_eq!(body, json!({"chat_id": FORUM, "name": name}));
    assert_eq!(create_forum_topic_body(FORUM, "Generale › Piano")["name"], "Generale › Piano");
}

#[test]
fn topic_changes_name_the_topic() {
    assert_eq!(
        edit_forum_topic_body(FORUM, 55, "Lavoro › Nuovo titolo"),
        json!({"chat_id": FORUM, "message_thread_id": 55, "name": "Lavoro › Nuovo titolo"})
    );
    assert_eq!(topic_body(FORUM, 55), json!({"chat_id": FORUM, "message_thread_id": 55}));
}

#[test]
fn the_bot_membership_is_asked_for_the_bot_itself() {
    assert_eq!(get_chat_member_body(FORUM, 777), json!({"chat_id": FORUM, "user_id": 777}));
}
