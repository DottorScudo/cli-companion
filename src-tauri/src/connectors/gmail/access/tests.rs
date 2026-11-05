use super::*;
use crate::connectors::gmail::descriptor::DESCRIPTOR;
use crate::connectors::testing::{test_context, TestContext};
use serde_json::json;

const CLIENT_ID: &str = "1234567890-abc.apps.googleusercontent.com";

fn context(config: Value) -> TestContext {
    test_context(&DESCRIPTOR, config)
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tauri::async_runtime::block_on(future)
}

#[test]
fn the_client_needs_a_valid_id_and_a_secret() {
    let t = context(json!({"client_id": CLIENT_ID}));
    assert!(matches!(client(&t.ctx), Err(ConnectorError::Auth(_))));
    t.ctx.set_secret(CLIENT_SECRET_SLOT, "  GOCSPX-segreto  ").unwrap();
    assert_eq!(client(&t.ctx).unwrap(), (CLIENT_ID.to_string(), "GOCSPX-segreto".to_string()));
}

#[test]
fn a_missing_or_malformed_client_id_is_a_configuration_error() {
    for config in [json!({}), json!({"client_id": "x"}), json!({"client_id": 5})] {
        let t = context(config);
        t.ctx.set_secret(CLIENT_SECRET_SLOT, "s").unwrap();
        assert!(matches!(client(&t.ctx), Err(ConnectorError::Config(_))));
    }
}

#[test]
fn a_cached_token_is_used_without_the_network() {
    let t = context(json!({"client_id": CLIENT_ID}));
    let cache = Mutex::new(TokenCache::new());
    let grant =
        TokenGrant { access_token: "ya29.x".to_string(), expires_in_secs: 3600, refresh_token: None, scope: None };
    remember(&cache, &t.ctx, &grant);
    assert_eq!(block_on(access_token(&cache, &t.ctx)).unwrap(), "ya29.x");
    forget(&cache, &t.ctx);
    assert!(matches!(block_on(access_token(&cache, &t.ctx)), Err(ConnectorError::Auth(_))));
}

#[test]
fn an_expired_token_needs_the_refresh_token() {
    let t = context(json!({"client_id": CLIENT_ID}));
    let cache = Mutex::new(TokenCache::new());
    let grant =
        TokenGrant { access_token: "ya29.x".to_string(), expires_in_secs: 3600, refresh_token: None, scope: None };
    remember(&cache, &t.ctx, &grant);
    t.clock.advance(3_600_000);
    assert_eq!(
        block_on(access_token(&cache, &t.ctx)),
        Err(ConnectorError::Auth("autorizzazione mancante".to_string()))
    );
}

#[test]
fn errors_never_contain_secrets() {
    let t = context(json!({"client_id": CLIENT_ID}));
    t.ctx.set_secret(CLIENT_SECRET_SLOT, "GOCSPX-segreto").unwrap();
    let cache = Mutex::new(TokenCache::new());
    let error = block_on(access_token(&cache, &t.ctx)).unwrap_err();
    assert!(!format!("{error:?}").contains("GOCSPX"));
}
