use super::*;

#[test]
fn the_external_id_joins_chat_and_message() {
    assert_eq!(external_id(42, 7), "42:7");
    assert_eq!(external_id(-100123, 1), "-100123:1");
}

#[test]
fn a_reply_target_must_be_in_the_same_chat() {
    assert_eq!(reply_message_id("42", Some("42:7")), Some(7));
    assert_eq!(reply_message_id("42", Some("43:7")), None);
    assert_eq!(reply_message_id("-5", Some("-5:12")), Some(12));
}

#[test]
fn malformed_reply_targets_are_ignored() {
    for raw in ["", "42", "42:", "42:x", "42:7:8", ":7", "42:99999999999999999999"] {
        assert_eq!(reply_message_id("42", Some(raw)), None, "{raw:?}");
    }
    assert_eq!(reply_message_id("42", None), None);
}
