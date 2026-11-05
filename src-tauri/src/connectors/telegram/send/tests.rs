use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::telegram::pacer::SLOT_GAP_MS;
use crate::connectors::testing::test_context;
use serde_json::json;
use tauri::async_runtime::block_on;

fn out(chat: &str, text: &str) -> OutboundMessage {
    OutboundMessage { chat: chat.to_string(), text: text.to_string(), reply_to_external: None }
}

#[test]
fn an_empty_text_sends_nothing_and_touches_no_network() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    for text in ["", "   ", "\n"] {
        assert_eq!(block_on(send(&pacer, &t.ctx, &out("42", text))).unwrap(), Vec::<String>::new());
    }
}

#[test]
fn an_invalid_chat_is_fatal() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    for chat in ["", "abc", "4 2", "99999999999999999999"] {
        let error = block_on(send(&pacer, &t.ctx, &out(chat, "ciao"))).unwrap_err();
        assert!(matches!(error, ConnectorError::Fatal(_)), "{chat:?}: {error}");
    }
}

#[test]
fn without_a_token_the_send_fails_with_auth_before_any_network_call() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    let error = block_on(send(&pacer, &t.ctx, &out("42", "ciao"))).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
}

#[test]
fn the_slot_wait_follows_the_pacer_and_is_capped() {
    let pacer = Mutex::new(Pacer::new());
    assert_eq!(reserve_slot(&pacer, "1", 0), Duration::ZERO);
    assert_eq!(reserve_slot(&pacer, "1", 0), Duration::from_millis(SLOT_GAP_MS as u64));
    assert_eq!(reserve_slot(&pacer, "2", 0), Duration::ZERO);
    let mut backwards = Pacer::new();
    backwards.reserve("3", i64::MAX);
    let capped = Mutex::new(backwards);
    assert!(reserve_slot(&capped, "3", i64::MIN) <= WAIT_MAX);
}

#[test]
fn a_poisoned_pacer_still_works() {
    let pacer = Mutex::new(Pacer::new());
    let _ = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _guard = pacer.lock().unwrap();
                panic!("lucchetto avvelenato");
            })
            .join()
    });
    assert_eq!(reserve_slot(&pacer, "1", 0), Duration::ZERO);
}
