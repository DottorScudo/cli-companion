use super::*;

#[test]
fn groups_have_negative_ids() {
    assert!(is_group("-5") && is_group("-1001234567890"));
    assert!(!is_group("42") && !is_group(""));
}

#[test]
fn only_supergroup_ids_have_topics() {
    assert!(is_supergroup("-1001234567890"));
    assert!(is_supergroup("-1001"));
    for chat in ["42", "-5", "-100", "-100x1", "-1001 ", "", "-", "0", "-1001234567890123456789", "ü\u{0}\n"] {
        assert!(!is_supergroup(chat), "{chat:?}");
    }
}

#[test]
fn a_short_title_is_kept_on_one_line() {
    assert_eq!(topic_name("Generale › Piano\nsettimanale"), "Generale › Piano settimanale");
}

#[test]
fn a_long_title_is_cut_at_128_units() {
    let name = topic_name(&"a".repeat(300));
    assert_eq!(name.chars().count(), TOPIC_NAME_UNITS_MAX);
}

#[test]
fn the_cut_never_splits_a_character() {
    // Ogni emoji vale 2 unita' UTF-16: ne stanno 64.
    let name = topic_name(&"😀".repeat(100));
    assert_eq!(name.chars().count(), 64);
    assert!(name.chars().all(|c| c == '😀'));
    let mixed = topic_name(&format!("{}😀", "a".repeat(127)));
    assert_eq!(mixed, "a".repeat(127), "l'emoji che sfora resta fuori intera");
    let accented = topic_name(&"è".repeat(200));
    assert_eq!(accented.chars().count(), TOPIC_NAME_UNITS_MAX);
}

#[test]
fn an_empty_title_has_a_fixed_name() {
    assert_eq!(topic_name(""), "Thread");
    assert_eq!(topic_name(" \n\u{200b} "), "Thread");
}

#[test]
fn topic_ids_are_positive_integers() {
    assert_eq!(topic_id("17"), Some(17));
    assert_eq!(topic_id(&subchat_of(99)), Some(99));
    for raw in ["", "0", "-3", "x", "1.5", "99999999999999999999"] {
        assert_eq!(topic_id(raw), None, "{raw:?}");
    }
}
