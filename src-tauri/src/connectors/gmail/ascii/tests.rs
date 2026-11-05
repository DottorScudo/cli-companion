use super::*;

#[test]
fn visible_ascii_within_the_limit_is_accepted() {
    assert!(is_visible_ascii("abc-123_~", 9));
    assert!(!is_visible_ascii("abc-123_~", 8));
}

#[test]
fn empty_spaces_controls_and_unicode_are_rejected() {
    for bad in ["", " ", "a b", "a\nb", "a\u{0}", "caffè", "😀"] {
        assert!(!is_visible_ascii(bad, 100), "{bad:?}");
    }
}
