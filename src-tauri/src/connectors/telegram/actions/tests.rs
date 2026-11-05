use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::testing::test_context;
use tauri::async_runtime::block_on;

fn bot() -> BotInfo {
    BotInfo { username: "MuseBot".to_string(), first_name: "Muse".to_string() }
}

#[test]
fn getme_is_parsed_into_the_bot_info() {
    let result = json!({"id": 1, "is_bot": true, "first_name": "Muse\u{0}", "username": "MuseBot"});
    assert_eq!(parse_bot(&result).unwrap(), bot());
}

#[test]
fn a_getme_without_a_safe_username_is_refused() {
    for result in [
        json!({}),
        json!(null),
        json!([]),
        json!({"first_name": "x"}),
        json!({"username": "MuseBot"}),
        json!({"first_name": "x", "username": "a/b"}),
        json!({"first_name": "x", "username": "a?start=1"}),
        json!({"first_name": "x", "username": 5}),
    ] {
        assert!(matches!(parse_bot(&result), Err(ConnectorError::Transient(_))), "{result}");
    }
}

#[test]
fn the_test_outcome_names_the_bot() {
    let outcome = test_outcome(&bot());
    assert_eq!(outcome.message, "Bot raggiungibile: @MuseBot (Muse).");
    assert_eq!(outcome.detail.as_deref(), Some("@MuseBot"));
}

#[test]
fn the_pair_outcome_gives_the_invite_link_and_the_manual_command() {
    let outcome = pair_outcome(&bot(), "AbC-123_x").unwrap();
    assert_eq!(outcome.detail.as_deref(), Some("https://t.me/MuseBot?start=AbC-123_x"));
    assert!(outcome.message.contains("/start AbC-123_x") && outcome.message.contains("@MuseBot"));
    assert!(outcome.message.contains("10 minuti"));
}

#[test]
fn a_code_that_is_not_url_safe_is_refused() {
    for code in ["", "a b", "a&b=1", "a/b", "é", &"a".repeat(65)] {
        assert!(matches!(pair_outcome(&bot(), code), Err(ConnectorError::Fatal(_))), "{code:?}");
    }
}

#[test]
fn an_undeclared_action_is_fatal() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let error = block_on(run(&t.ctx, ActionId::Authorize)).unwrap_err();
    assert_eq!(error, ConnectorError::Fatal("azione non dichiarata".to_string()));
}

#[test]
fn pairing_without_a_code_is_fatal_before_any_network_call() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let error = block_on(run(&t.ctx, ActionId::Pair)).unwrap_err();
    assert!(matches!(error, ConnectorError::Fatal(_)), "{error}");
}

#[test]
fn the_test_action_without_a_token_is_an_auth_error_before_any_network_call() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let error = block_on(run(&t.ctx, ActionId::Test)).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
    assert!(t.ctx.state_get(BOT_USERNAME_KEY).is_none());
}
