use super::*;
use serde_json::json;

fn profile() -> Profile {
    parse_profile(&json!({"emailAddress": "mario@gmail.com", "messagesTotal": 10, "historyId": "5"})).unwrap()
}

#[test]
fn the_email_address_is_read() {
    assert_eq!(profile().email, "mario@gmail.com");
}

#[test]
fn bad_profiles_are_rejected() {
    let long = format!("{}@x.it", "a".repeat(260));
    for v in [
        json!({}),
        json!(null),
        json!([]),
        json!({"emailAddress": 5}),
        json!({"emailAddress": ""}),
        json!({"emailAddress": "nochiocciola"}),
        json!({"emailAddress": "a b@c.it"}),
        json!({"emailAddress": "a@c.it\n"}),
        json!({"emailAddress": "\u{0}@c.it"}),
        json!({"emailAddress": long}),
    ] {
        assert!(matches!(parse_profile(&v), Err(ConnectorError::Transient(_))), "{v}");
    }
}

#[test]
fn the_length_bounds_are_inclusive() {
    assert!(parse_profile(&json!({"emailAddress": "a@b"})).is_ok());
    assert!(parse_profile(&json!({"emailAddress": "@b"})).is_err());
    let max = format!("{}@b", "a".repeat(EMAIL_MAX - 2));
    assert!(parse_profile(&json!({"emailAddress": max})).is_ok());
    let over = format!("{}@b", "a".repeat(EMAIL_MAX - 1));
    assert!(parse_profile(&json!({"emailAddress": over})).is_err());
}

#[test]
fn unicode_addresses_are_accepted() {
    assert!(parse_profile(&json!({"emailAddress": "mario@caffè.it"})).is_ok());
}

#[test]
fn the_outcomes_name_the_account() {
    let test = test_outcome(&profile());
    assert_eq!(test.message, "Account Gmail raggiungibile: mario@gmail.com.");
    assert_eq!(test.detail.as_deref(), Some("mario@gmail.com"));
    let auth = authorize_outcome(&profile());
    assert_eq!(auth.message, "Autorizzazione completata per mario@gmail.com.");
    assert_eq!(auth.detail.as_deref(), Some("mario@gmail.com"));
}
