use super::*;
use serde_json::json;

#[test]
fn the_name_is_first_and_last_name() {
    assert_eq!(sender_name(&json!({"first_name": "Mario", "last_name": "Rossi"})), "Mario Rossi");
    assert_eq!(sender_name(&json!({"first_name": "Mario"})), "Mario");
    assert_eq!(sender_name(&json!({"last_name": "Rossi"})), "Rossi");
}

#[test]
fn without_a_name_the_username_is_used() {
    assert_eq!(sender_name(&json!({"username": "mario_r"})), "@mario_r");
    assert_eq!(sender_name(&json!({"first_name": "  ", "username": "mario_r"})), "@mario_r");
    assert_eq!(sender_name(&json!({"username": "no spazi"})), "");
}

#[test]
fn an_unknown_sender_has_an_empty_name() {
    for from in [json!({}), json!(null), json!([]), json!("x"), json!({"first_name": 5, "username": null})] {
        assert_eq!(sender_name(&from), "");
    }
}

#[test]
fn control_and_invisible_characters_are_removed() {
    let from = json!({"first_name": "Ma\u{0}rio\n\t", "last_name": "\u{202e}Ro\u{200b}ssi"});
    assert_eq!(sender_name(&from), "Mario Rossi");
}

#[test]
fn the_name_is_at_most_sixty_characters() {
    let emoji = sender_name(&json!({"first_name": "😀".repeat(100)}));
    assert_eq!(emoji.chars().count(), SENDER_NAME_MAX);
    let long = sender_name(&json!({"first_name": "a".repeat(500)}));
    assert_eq!(long.chars().count(), SENDER_NAME_MAX);
}

#[test]
fn clean_display_trims_and_caps() {
    assert_eq!(clean_display("  ciao  ", 10), "ciao");
    assert_eq!(clean_display("abcdef", 3), "abc");
    assert_eq!(clean_display("\u{0}\u{1}", 3), "");
}

#[test]
fn usernames_are_url_safe() {
    assert!(valid_username("MuseBot"));
    assert!(valid_username("a_1"));
    assert!(valid_username(&"a".repeat(64)));
    for bad in ["", "a b", "a/b", "a?x=1", "a#", "é", "a-b", "../x", &"a".repeat(65)] {
        assert!(!valid_username(bad), "{bad:?}");
    }
}
