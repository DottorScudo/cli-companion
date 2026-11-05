use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::testing::test_context;
use serde_json::json;
use tauri::async_runtime::block_on;

#[test]
fn without_a_token_the_download_fails_with_auth_before_any_network_call() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let file = RemoteFile { id: "AgAD".to_string(), name: "foto-1.jpg".to_string(), size: None };
    let error = block_on(fetch_file(&t.ctx, &file)).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
}
