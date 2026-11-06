use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::telegram::pacer::Pacer;
use crate::connectors::testing::test_context;
use serde_json::json;
use tauri::async_runtime::block_on;

#[test]
fn the_long_poll_fits_inside_the_runtime_limits() {
    assert!(POLL_HTTP_TIMEOUT > Duration::from_secs(POLL_WAIT_SECS));
    assert!(POLL_HTTP_TIMEOUT < RECEIVE_TIMEOUT);
    assert_eq!(POLL_HTTP_TIMEOUT, Duration::from_secs(60));
}

#[test]
fn without_a_token_both_the_first_run_and_a_normal_round_fail_with_auth() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    for cursor in [None, Some(json!({"offset": 5})), Some(json!("rotto"))] {
        let error = block_on(receive(&pacer, &t.ctx, cursor)).unwrap_err();
        assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
    }
}
