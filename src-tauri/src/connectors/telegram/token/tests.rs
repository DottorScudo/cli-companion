use super::*;
use crate::connectors::descriptor::HubCapability;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::testing::test_context;
use serde_json::json;

const SECRET: &str = "AAEhBOweDXYZabcdefghijklmnopqrstuv_-12";

fn token() -> String {
    format!("123456789:{SECRET}")
}

#[test]
fn a_well_formed_token_is_accepted() {
    assert!(valid_token(&token()));
    assert!(valid_token(&format!("1:{}", "a".repeat(SECRET_MIN))));
}

#[test]
fn malformed_tokens_are_rejected() {
    let long = format!("1:{}", "a".repeat(SECRET_MAX + 1));
    let short = format!("1:{}", "a".repeat(SECRET_MIN - 1));
    for bad in [
        "",
        ":",
        "abc",
        "123456789",
        "123456789:",
        "x:AAEhBOweDXYZabcdefghijklmnopqrstuv",
        ":AAEhBOweDXYZabcdefghijklmnopqrstuv",
        "123:AAEhBOweDXYZ abcdefghijklmnopqrstuv",
        "123:AAEhBOweDXYZ/abcdefghijklmnopqrstuv",
        "123:AAEhBOweDXYZ?abcdefghijklmnopqrstuv",
        "12345678901234567890123:AAEhBOweDXYZabcdefghijklmnopqrstuv",
        "123:ünïcodeAAEhBOweDXYZabcdefghijklmnopqrstuv",
        long.as_str(),
        short.as_str(),
    ] {
        assert!(!valid_token(bad), "{bad:?}");
    }
}

#[test]
fn urls_embed_the_token_in_the_path() {
    assert_eq!(method_url("1:abc", "getMe"), "https://api.telegram.org/bot1:abc/getMe");
    assert_eq!(file_url("1:abc", "photos/a.jpg"), "https://api.telegram.org/file/bot1:abc/photos/a.jpg");
}

#[test]
fn a_missing_token_is_an_auth_error_without_the_secret() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let error = bot_token(&t.ctx).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
}

#[test]
fn a_malformed_token_is_an_auth_error_that_does_not_leak_it() {
    let t = test_context(&DESCRIPTOR, json!({}));
    t.ctx.set_secret(TOKEN_SLOT, "segreto-malformato").unwrap();
    let error = bot_token(&t.ctx).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)));
    assert!(!error.to_string().contains("segreto-malformato"));
}

#[test]
fn a_valid_token_is_returned_trimmed() {
    let t = test_context(&DESCRIPTOR, json!({}));
    t.ctx.set_secret(TOKEN_SLOT, &format!("  {}\n", token())).unwrap();
    assert_eq!(bot_token(&t.ctx).unwrap(), token());
}

#[test]
fn the_descriptor_declares_the_token_slot_and_host() {
    assert!(DESCRIPTOR.declares_slot(TOKEN_SLOT));
    assert!(DESCRIPTOR.permissions.hosts.contains(&API_HOST));
    assert!(DESCRIPTOR.has_capability(HubCapability::ReceiveNotices));
}
