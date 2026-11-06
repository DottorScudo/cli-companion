use super::*;
use crate::connectors::telegram::descriptor::DESCRIPTOR;
use crate::connectors::testing::{inbound, test_context};
use serde_json::json;
use tauri::async_runtime::block_on;

const FORUM: &str = "-1001234567890";

fn admin(can_manage_topics: Value) -> Value {
    json!({"status": "administrator", "user": {"id": 777, "is_bot": true}, "can_manage_topics": can_manage_topics})
}

fn invite_from(chat: &str) -> InboundMessage {
    InboundMessage {
        chat: chat.to_string(),
        pairing_code: Some("CODICE".to_string()),
        ..inbound(&format!("{chat}:3"), "42", "/start CODICE")
    }
}

#[test]
fn a_bot_without_the_rights_answers_with_the_instructions() {
    for member in [
        json!({"status": "member", "user": {"id": 777}}),
        json!({"status": "restricted", "can_manage_topics": true}),
        admin(json!(false)),
        admin(json!("true")),
        json!({"status": "administrator"}),
        json!({"status": "left"}),
        json!(null),
        json!([]),
    ] {
        assert!(!can_manage_topics(&member), "{member}");
    }
    assert_eq!(admission(Ok(false)).unwrap(), Admission::Refuse(MISSING_RIGHTS_TEXT));
    assert!(MISSING_RIGHTS_TEXT.contains("amministratore") && MISSING_RIGHTS_TEXT.contains("«Gestire i topic»"));
    assert!(MISSING_RIGHTS_TEXT.contains("non è stato usato"));
}

#[test]
fn an_administrator_that_manages_topics_lets_the_code_through() {
    assert!(can_manage_topics(&admin(json!(true))));
    assert_eq!(admission(Ok(true)).unwrap(), Admission::Pass);
}

#[test]
fn a_failed_check_keeps_the_code_and_says_so() {
    let unverified = admission(Err(ConnectorError::Transient("rete".to_string()))).unwrap();
    assert_eq!(unverified, Admission::Refuse(UNVERIFIED_TEXT));
    assert_eq!(admission(Err(ConnectorError::SubchatGone)).unwrap(), Admission::Refuse(UNVERIFIED_TEXT));
    let auth = admission(Err(ConnectorError::Auth("token".to_string()))).unwrap_err();
    assert!(matches!(auth, ConnectorError::Auth(_)));
}

#[test]
fn the_bot_id_comes_from_getme() {
    assert_eq!(bot_id(&json!({"id": 777, "is_bot": true})), Some(777));
    for me in [json!({}), json!({"id": "777"}), json!({"id": -1}), json!({"id": 0}), json!(null)] {
        assert_eq!(bot_id(&me), None, "{me}");
    }
}

#[test]
fn private_invites_and_plain_group_messages_pass_without_any_network_call() {
    // Senza token ogni chiamata fallirebbe con `Auth`: qui non ne parte nessuna.
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    let in_group = InboundMessage { chat: FORUM.to_string(), ..inbound(&format!("{FORUM}:2"), "42", "ciao") };
    let messages = vec![invite_from("42"), in_group];
    assert_eq!(block_on(screen(&pacer, &t.ctx, messages.clone())).unwrap(), messages);
}

#[test]
fn an_invite_from_a_group_is_checked_and_a_token_error_stops_the_round() {
    let t = test_context(&DESCRIPTOR, json!({}));
    let pacer = Mutex::new(Pacer::new());
    let error = block_on(screen(&pacer, &t.ctx, vec![invite_from(FORUM)])).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)), "{error}");
}
