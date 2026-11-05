use super::*;
use crate::connectors::gmail::mime::AttachmentSource;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;

fn b64(text: &str) -> String {
    URL_SAFE_NO_PAD.encode(text.as_bytes())
}

fn message(payload: Value) -> Value {
    json!({"id": "18c3f", "threadId": "18c3a", "internalDate": "1700000000123", "snippet": "anteprima &amp; co", "payload": payload})
}

fn headers(subject: &str, from: &str, date: &str) -> Value {
    json!([
        {"name": "Subject", "value": subject},
        {"name": "from", "value": from},
        {"name": "DATE", "value": date}
    ])
}

#[test]
fn a_plain_text_message_is_parsed() {
    let v = message(json!({
        "mimeType": "text/plain",
        "headers": headers("Riunione", "Mario Rossi <mario@gmail.com>", "Tue, 14 Nov 2023 22:13:20 +0000"),
        "body": {"size": 5, "data": b64("Ciao\r\nmondo\r\n")}
    }));
    let m = parse_message(&v).unwrap();
    assert_eq!(m.id, "18c3f");
    assert_eq!(m.thread_id.as_deref(), Some("18c3a"));
    assert_eq!(m.internal_ms, 1_700_000_000_123);
    assert_eq!(m.subject, "Riunione");
    assert_eq!(m.from, "Mario Rossi <mario@gmail.com>");
    assert_eq!(m.date, "Tue, 14 Nov 2023 22:13:20 +0000");
    assert_eq!(m.text, "Ciao\nmondo");
    assert!(m.attachments.is_empty());
}

#[test]
fn headers_are_decoded_and_cleaned() {
    let v = message(json!({
        "mimeType": "text/plain",
        "headers": headers("=?UTF-8?B?Y2FmZsOo?=\r\n =?utf-8?Q?_freddo?=", "=?iso-8859-1?Q?Jos=E9?= <j@x.es>", "  x\ny "),
        "body": {"data": b64("t")}
    }));
    let m = parse_message(&v).unwrap();
    assert_eq!(m.subject, "caffè freddo");
    assert_eq!(m.from, "José <j@x.es>");
    assert_eq!(m.date, "x y");
}

#[test]
fn html_only_messages_are_converted() {
    let v = message(json!({
        "mimeType": "multipart/alternative",
        "parts": [{"mimeType": "text/html", "body": {"data": b64("<p>Ciao <b>mondo</b></p><p>a &amp; b</p>")}}]
    }));
    assert_eq!(parse_message(&v).unwrap().text, "Ciao mondo\n\na & b");
}

#[test]
fn plain_wins_over_html_unless_blank() {
    let both = |plain: &str| {
        message(json!({"mimeType": "multipart/alternative", "parts": [
            {"mimeType": "text/plain", "body": {"data": b64(plain)}},
            {"mimeType": "text/html", "body": {"data": b64("<p>html</p>")}}]}))
    };
    assert_eq!(parse_message(&both("semplice")).unwrap().text, "semplice");
    assert_eq!(parse_message(&both("  \n ")).unwrap().text, "html");
}

#[test]
fn the_snippet_is_the_last_resort() {
    let v = message(json!({"mimeType": "multipart/mixed", "parts": []}));
    assert_eq!(parse_message(&v).unwrap().text, "anteprima & co");
    let none = json!({"id": "a1", "internalDate": "5"});
    let m = parse_message(&none).unwrap();
    assert_eq!(m.text, "");
    assert_eq!(m.subject, "");
    assert!(m.thread_id.is_none());
}

#[test]
fn attachments_are_listed() {
    let v = message(json!({"mimeType": "multipart/mixed", "parts": [
        {"mimeType": "text/plain", "body": {"data": b64("corpo")}},
        {"mimeType": "application/pdf", "filename": "a.pdf", "body": {"attachmentId": "ATT", "size": 100}}]}));
    let m = parse_message(&v).unwrap();
    assert_eq!(m.attachments.len(), 1);
    assert_eq!(m.attachments[0].source, AttachmentSource::Remote("ATT".to_string()));
}

#[test]
fn the_text_is_capped_and_cleaned() {
    let v = message(json!({"mimeType": "text/plain", "body": {"data": b64(&"è".repeat(100_000))}}));
    assert_eq!(parse_message(&v).unwrap().text.chars().count(), TEXT_MAX);
    let dirty = message(json!({"mimeType": "text/plain", "body": {"data": b64("a\u{0}b\u{7}c\td\re")}}));
    assert_eq!(parse_message(&dirty).unwrap().text, "abc\td\ne");
}

#[test]
fn a_huge_subject_is_cut_on_a_char_boundary() {
    let v = message(json!({"headers": headers(&"😀".repeat(5000), "", ""), "mimeType": "text/plain"}));
    assert_eq!(parse_message(&v).unwrap().subject.chars().count(), HEADER_MAX);
}

#[test]
fn id_and_internal_date_are_required() {
    for v in [
        json!({}),
        json!(null),
        json!([]),
        json!({"internalDate": "5"}),
        json!({"id": "a1"}),
        json!({"id": "a/1", "internalDate": "5"}),
        json!({"id": "", "internalDate": "5"}),
        json!({"id": "a1", "internalDate": "ieri"}),
        json!({"id": "a1", "internalDate": null}),
        json!({"id": 7, "internalDate": "5"}),
    ] {
        assert!(matches!(parse_message(&v), Err(ConnectorError::Transient(_))), "{v}");
    }
}

#[test]
fn a_numeric_internal_date_and_a_bad_thread_id_are_tolerated() {
    let m = parse_message(&json!({"id": "a1", "internalDate": 42, "threadId": "t/../1"})).unwrap();
    assert_eq!(m.internal_ms, 42);
    assert!(m.thread_id.is_none());
}

#[test]
fn malformed_payloads_do_not_panic() {
    for payload in
        [json!(null), json!("x"), json!([]), json!({"headers": "x", "parts": 3}), json!({"headers": [1, null]})]
    {
        let v = json!({"id": "a1", "internalDate": "1", "payload": payload});
        assert!(parse_message(&v).is_ok());
    }
}
