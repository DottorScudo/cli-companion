use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::testing::test_context;
use serde_json::json;
use tauri::async_runtime::block_on;

const FORUM: &str = "-1001234567890";

#[test]
fn an_invalid_chat_is_fatal_and_an_invalid_topic_is_gone_before_any_network_call() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    assert!(matches!(block_on(open(&pacer, &t.ctx, "abc", "x")).unwrap_err(), ConnectorError::Fatal(_)));
    assert!(matches!(block_on(close(&pacer, &t.ctx, "", "5")).unwrap_err(), ConnectorError::Fatal(_)));
    assert_eq!(block_on(rename(&pacer, &t.ctx, FORUM, "x", "t")).unwrap_err(), ConnectorError::SubchatGone);
    assert_eq!(block_on(close(&pacer, &t.ctx, FORUM, "0")).unwrap_err(), ConnectorError::SubchatGone);
    assert_eq!(block_on(reopen(&pacer, &t.ctx, FORUM, "0")).unwrap_err(), ConnectorError::SubchatGone);
    assert_eq!(block_on(delete(&pacer, &t.ctx, FORUM, "-2")).unwrap_err(), ConnectorError::SubchatGone);
}

#[test]
fn without_a_token_every_topic_call_fails_with_auth() {
    let t = test_context(&DESCRIPTOR, json!({}));
    // Un pacer nuovo per chiamata: nessuna attesa del turno del gruppo.
    let pacer = || Mutex::new(Pacer::new());
    let errors = [
        block_on(open(&pacer(), &t.ctx, FORUM, "Generale › Piano")).map(drop).unwrap_err(),
        block_on(rename(&pacer(), &t.ctx, FORUM, "5", "Nuovo")).unwrap_err(),
        block_on(close(&pacer(), &t.ctx, FORUM, "5")).unwrap_err(),
        block_on(reopen(&pacer(), &t.ctx, FORUM, "5")).unwrap_err(),
        block_on(delete(&pacer(), &t.ctx, FORUM, "5")).unwrap_err(),
    ];
    for error in errors {
        assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
    }
}
