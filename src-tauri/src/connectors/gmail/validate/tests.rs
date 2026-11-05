use super::*;
use serde_json::json;

#[test]
fn reasonable_queries_are_accepted() {
    for q in ["is:unread", "  from:(a@b.it OR c@d.it) label:inbox ", "subject:\"(ciao\"", "caffè 😀", "a\"b\"c"] {
        assert_eq!(validate_query(q), Ok(()), "{q:?}");
    }
}

#[test]
fn empty_and_blank_queries_are_rejected() {
    assert!(validate_query("").is_err());
    assert!(validate_query("   \t ").is_err());
}

#[test]
fn the_length_limit_counts_characters() {
    assert!(validate_query(&"a".repeat(QUERY_MAX)).is_ok());
    assert!(validate_query(&"a".repeat(QUERY_MAX + 1)).is_err());
    assert!(validate_query(&"è".repeat(QUERY_MAX)).is_ok());
    assert!(validate_query(&"a".repeat(100_000)).is_err());
}

#[test]
fn multi_line_and_control_characters_are_rejected() {
    for q in ["a\nb", "a\r\nb", "a\tb", "a\u{0}b", "a\u{7f}"] {
        assert!(validate_query(q).is_err(), "{q:?}");
    }
}

#[test]
fn parentheses_must_balance_outside_quotes() {
    for bad in ["a)", ")(", "(a", "((a)", "a) OR (b", "\"a\") after:0 (\""] {
        assert!(validate_query(bad).is_err(), "{bad:?}");
    }
    assert!(validate_query("(a (b) c)").is_ok());
    assert!(validate_query("\"a)\" (b)").is_ok());
}

#[test]
fn quotes_must_be_even() {
    assert!(validate_query("\"a").is_err());
    assert!(validate_query("a\"b\"c\"").is_err());
    assert!(validate_query("\"\"").is_ok());
}

#[test]
fn the_filter_needs_a_string_query() {
    assert!(validate_filter(&json!({"query": "is:unread"})).is_ok());
    for bad in [json!({}), json!(null), json!({"query": 5}), json!({"query": ""}), json!([]), json!("x")] {
        assert!(validate_filter(&bad).is_err(), "{bad}");
    }
}

#[test]
fn client_ids_are_ascii_with_the_google_suffix() {
    assert!(validate_client_id("123-abc.apps.googleusercontent.com").is_ok());
    assert!(validate_client_id("  123-abc.apps.googleusercontent.com ").is_ok());
    for bad in [
        "",
        "123-abc",
        ".apps.googleusercontent.com",
        "12 3.apps.googleusercontent.com",
        "è.apps.googleusercontent.com",
        "a\n.apps.googleusercontent.com",
    ] {
        assert!(validate_client_id(bad).is_err(), "{bad:?}");
    }
    let long = format!("{}{CLIENT_ID_SUFFIX}", "a".repeat(CLIENT_ID_MAX));
    assert!(validate_client_id(&long).is_err());
}

#[test]
fn the_config_needs_a_valid_client_id() {
    assert!(validate_config(&json!({"client_id": "x.apps.googleusercontent.com"})).is_ok());
    for bad in [json!({}), json!({"client_id": 1}), json!({"client_id": "x"}), json!(null)] {
        assert!(validate_config(&bad).is_err(), "{bad}");
    }
}
