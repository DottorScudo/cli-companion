use super::*;
use serde_json::json;

fn body(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn error_body(code: u64, description: &str) -> Vec<u8> {
    body(&json!({"ok": false, "error_code": code, "description": description}))
}

#[test]
fn an_ok_response_yields_its_result() {
    let ok = body(&json!({"ok": true, "result": {"id": 7}}));
    assert_eq!(classify(200, &ok).unwrap(), json!({"id": 7}));
    let list = body(&json!({"ok": true, "result": []}));
    assert_eq!(classify(200, &list).unwrap(), json!([]));
}

#[test]
fn unauthorized_and_not_found_stop_with_auth() {
    for code in [401u16, 404] {
        let error = classify(code, &error_body(u64::from(code), "Unauthorized")).unwrap_err();
        assert!(matches!(error, ConnectorError::Auth(_)), "{code}: {error}");
    }
    // Anche con un corpo che non e' JSON conta lo stato.
    assert!(matches!(classify(401, b"<html>").unwrap_err(), ConnectorError::Auth(_)));
}

#[test]
fn too_many_requests_carry_the_retry_after() {
    let limited = body(&json!({
        "ok": false, "error_code": 429, "description": "Too Many Requests: retry after 7",
        "parameters": {"retry_after": 7}
    }));
    assert_eq!(
        classify(429, &limited).unwrap_err(),
        ConnectorError::RateLimited { retry_after: Some(Duration::from_secs(7)) }
    );
    assert_eq!(classify(429, b"").unwrap_err(), ConnectorError::RateLimited { retry_after: None });
    let odd = body(&json!({"ok": false, "error_code": 429, "parameters": {"retry_after": "molto"}}));
    assert_eq!(classify(429, &odd).unwrap_err(), ConnectorError::RateLimited { retry_after: None });
}

#[test]
fn conflicts_forbidden_bad_requests_and_server_errors_are_retried() {
    for code in [400u16, 403, 409, 500, 502, 503, 418] {
        let error = classify(code, &error_body(u64::from(code), "x")).unwrap_err();
        assert!(matches!(error, ConnectorError::Transient(_)), "{code}: {error}");
        assert!(!error.stops_the_supervisor());
    }
}

#[test]
fn unexpected_shapes_are_transient() {
    for (status, raw) in [
        (200u16, b"".as_slice()),
        (200, b"non json"),
        (200, b"null"),
        (200, b"[]"),
        (200, br#"{"ok": true}"#),
        (200, br#"{"ok": "si", "result": 1}"#),
        (200, br#"{"ok": false}"#),
        (500, b"<html>Bad gateway</html>"),
    ] {
        let error = classify(status, raw).unwrap_err();
        assert!(matches!(error, ConnectorError::Transient(_)), "{status} {raw:?}: {error}");
    }
}

#[test]
fn the_json_error_code_wins_over_a_generic_status() {
    let error = classify(200, &error_body(401, "x")).unwrap_err();
    assert!(matches!(error, ConnectorError::Auth(_)));
}

#[test]
fn a_success_status_with_ok_false_is_never_a_result() {
    assert!(classify(200, &body(&json!({"ok": false, "result": 1}))).is_err());
}

#[test]
fn the_description_never_leaks() {
    for code in [400u16, 401, 403, 404, 409, 429, 500] {
        let raw = body(&json!({
            "ok": false, "error_code": code, "description": "Bad Request: 123456:SECRET-TOKEN-VALUE"
        }));
        let error = classify(code, &raw).unwrap_err();
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains("SECRET") && !shown.contains("Bad Request"), "{shown}");
    }
}

#[test]
fn error_of_never_returns_a_result() {
    assert!(matches!(error_of(200, b"{}"), ConnectorError::Transient(_)));
    assert!(matches!(error_of(404, b"binario\xff"), ConnectorError::Auth(_)));
}

#[test]
fn a_deleted_topic_is_a_gone_subchat() {
    for description in ["Bad Request: message thread not found", "Bad Request: TOPIC_ID_INVALID"] {
        assert_eq!(
            classify(400, &error_body(400, description)).unwrap_err(),
            ConnectorError::SubchatGone,
            "{description}"
        );
    }
    // Il codice del JSON vale anche con uno stato HTTP diverso.
    assert_eq!(
        classify(200, &error_body(400, "Bad Request: message thread not found")).unwrap_err(),
        ConnectorError::SubchatGone
    );
}

#[test]
fn other_bad_requests_stay_transient() {
    for description in [
        "Bad Request: chat not found",
        "Bad Request: TOPIC_CLOSED",
        "Bad Request: not enough rights to create a topic",
        "Bad Request: the chat is not a forum",
        "",
    ] {
        let error = classify(400, &error_body(400, description)).unwrap_err();
        assert!(matches!(error, ConnectorError::Transient(_)), "{description}: {error}");
    }
    let without_description = body(&json!({"ok": false, "error_code": 400}));
    assert!(matches!(classify(400, &without_description).unwrap_err(), ConnectorError::Transient(_)));
    // Lo stesso testo con un altro codice non e' un topic sparito.
    let forbidden = classify(403, &error_body(403, "message thread not found")).unwrap_err();
    assert!(matches!(forbidden, ConnectorError::Transient(_)));
}

#[test]
fn unreadable_html_is_recognised_for_the_plain_fallback() {
    for description in [
        "Bad Request: can't parse entities: Unsupported start tag \"x\" at byte offset 3",
        "Bad Request: Can't parse entities: unclosed start tag",
    ] {
        assert!(entities_rejected(400, &error_body(400, description)), "{description}");
        assert!(entities_rejected(200, &error_body(400, description)), "il codice del JSON vale: {description}");
    }
}

#[test]
fn other_errors_are_not_an_html_rejection() {
    let parse = "Bad Request: can't parse entities";
    assert!(!entities_rejected(403, &error_body(403, parse)));
    assert!(!entities_rejected(429, &error_body(429, parse)));
    assert!(!entities_rejected(400, &error_body(400, "Bad Request: message thread not found")));
    assert!(!entities_rejected(400, &error_body(400, "Bad Request: chat not found")));
    assert!(!entities_rejected(400, b"<html>"));
    assert!(!entities_rejected(200, &body(&json!({"ok": true, "result": {"message_id": 1}}))));
    // Il topic sparito resta `SubchatGone`, anche con il ripiego dell'HTML.
    assert_eq!(
        classify(400, &error_body(400, "Bad Request: message thread not found")).unwrap_err(),
        ConnectorError::SubchatGone
    );
}

#[test]
fn an_html_rejection_that_is_classified_never_leaks_its_description() {
    let raw = error_body(400, "Bad Request: can't parse entities: 123456:SECRET-TOKEN-VALUE");
    let error = classify(400, &raw).unwrap_err();
    let shown = format!("{error} {error:?}");
    assert!(matches!(error, ConnectorError::Transient(_)));
    assert!(!shown.contains("SECRET") && !shown.contains("parse"), "{shown}");
}
