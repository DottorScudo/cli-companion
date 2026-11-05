use super::*;
use crate::connectors::gmail::keys::API_HOST;

#[test]
fn the_base_url_uses_the_declared_host() {
    assert!(API_BASE.starts_with(&format!("https://{API_HOST}/")));
}

#[test]
fn message_ids_use_a_safe_alphabet() {
    assert!(valid_id("18c3f_A-9"));
    assert!(valid_id(&"a".repeat(256)));
    for bad in ["", "a/b", "a b", "a?b", "..", "a.b", "è", "a\n", &"a".repeat(257)] {
        assert!(!valid_id(bad), "{bad:?}");
    }
}

#[test]
fn attachment_ids_may_be_longer() {
    assert!(valid_attachment_id(&"A".repeat(2048)));
    assert!(!valid_attachment_id(&"A".repeat(2049)));
    assert!(!valid_attachment_id(""));
    assert!(!valid_attachment_id("a/../b"));
    assert!(!valid_attachment_id("a=b"));
}

#[test]
fn the_list_url_encodes_query_and_token() {
    let url = list_url("(from:a@b.it) after:100", None);
    assert_eq!(
        url,
        "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=%28from%3Aa%40b.it%29%20after%3A100&maxResults=100"
    );
    let paged = list_url("x&y", Some("tok/en=="));
    assert!(paged.ends_with("&pageToken=tok%2Fen%3D%3D"), "{paged}");
    assert!(paged.contains("q=x%26y&"));
}

#[test]
fn message_and_attachment_urls_reject_bad_ids() {
    assert_eq!(
        message_url("abc123").as_deref(),
        Some("https://gmail.googleapis.com/gmail/v1/users/me/messages/abc123?format=full")
    );
    assert_eq!(message_url("../x"), None);
    assert_eq!(message_url(""), None);
    assert_eq!(
        attachment_url("m1", "att_-9").as_deref(),
        Some("https://gmail.googleapis.com/gmail/v1/users/me/messages/m1/attachments/att_-9")
    );
    assert_eq!(attachment_url("m/1", "a"), None);
    assert_eq!(attachment_url("m1", "a/b"), None);
    assert_eq!(attachment_url("m1", ""), None);
}

#[test]
fn the_profile_url_is_fixed() {
    assert_eq!(profile_url(), "https://gmail.googleapis.com/gmail/v1/users/me/profile");
}

#[test]
fn the_search_query_wraps_the_trimmed_user_query() {
    assert_eq!(search_query("  is:unread  ", 1_700_000_000), "(is:unread) after:1700000000");
    assert_eq!(search_query("a OR b", 0), "(a OR b) after:0");
    assert_eq!(search_query("", 5), "() after:5");
    assert_eq!(search_query("x", -3), "(x) after:-3");
}
