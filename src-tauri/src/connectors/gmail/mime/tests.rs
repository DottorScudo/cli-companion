use super::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;

fn b64(text: &str) -> String {
    URL_SAFE_NO_PAD.encode(text.as_bytes())
}

fn text_part(mime: &str, text: &str) -> Value {
    json!({"mimeType": mime, "filename": "", "headers": [{"name": "Content-Type", "value": format!("{mime}; charset=\"UTF-8\"")}],
           "body": {"size": text.len(), "data": b64(text)}})
}

#[test]
fn a_single_part_message_is_its_own_text() {
    let payload = json!({"mimeType": "text/plain", "filename": "", "body": {"size": 4, "data": b64("ciao")}});
    let got = collect(&payload);
    assert_eq!(got.plain.as_deref(), Some("ciao"));
    assert!(got.html.is_none() && got.attachments.is_empty());
}

#[test]
fn multipart_alternative_gives_both_texts() {
    let payload = json!({"mimeType": "multipart/alternative", "parts": [
        text_part("text/plain", "semplice"), text_part("text/html", "<b>ricco</b>")]});
    let got = collect(&payload);
    assert_eq!(got.plain.as_deref(), Some("semplice"));
    assert_eq!(got.html.as_deref(), Some("<b>ricco</b>"));
}

#[test]
fn the_first_text_part_wins() {
    let payload = json!({"mimeType": "multipart/mixed", "parts": [
        text_part("text/plain", "primo"), text_part("text/plain", "secondo")]});
    assert_eq!(collect(&payload).plain.as_deref(), Some("primo"));
}

#[test]
fn nested_multiparts_with_attachments_are_walked() {
    let payload = json!({"mimeType": "multipart/mixed", "parts": [
        {"mimeType": "multipart/alternative", "parts": [text_part("text/plain", "corpo")]},
        {"mimeType": "application/pdf", "filename": "fattura.pdf", "body": {"attachmentId": "ATT_1-x", "size": 12345}},
        {"mimeType": "image/png", "filename": "logo.png", "body": {"size": 3, "data": URL_SAFE_NO_PAD.encode([1u8, 2, 3])}}
    ]});
    let got = collect(&payload);
    assert_eq!(got.plain.as_deref(), Some("corpo"));
    assert_eq!(got.attachments.len(), 2);
    assert_eq!(got.attachments[0].filename, "fattura.pdf");
    assert_eq!(got.attachments[0].size, 12345);
    assert_eq!(got.attachments[0].source, AttachmentSource::Remote("ATT_1-x".to_string()));
    assert_eq!(got.attachments[1].source, AttachmentSource::Inline(vec![1, 2, 3]));
    assert_eq!(got.attachments[1].size, 3);
}

#[test]
fn a_text_attachment_is_not_the_body() {
    let mut named = text_part("text/plain", "dentro un file");
    named["filename"] = json!("note.txt");
    let mut disposed = text_part("text/plain", "disposition");
    disposed["headers"] = json!([{"name": "content-disposition", "value": "Attachment; filename=x"}]);
    let payload =
        json!({"mimeType": "multipart/mixed", "parts": [named, disposed, text_part("text/plain", "vero corpo")]});
    let got = collect(&payload);
    assert_eq!(got.plain.as_deref(), Some("vero corpo"));
    assert_eq!(got.attachments.len(), 1);
    assert_eq!(got.attachments[0].filename, "note.txt");
}

#[test]
fn attachments_without_content_or_id_are_skipped_and_names_are_cleaned() {
    let payload = json!({"mimeType": "multipart/mixed", "parts": [
        {"mimeType": "x/y", "filename": "vuoto.bin", "body": {"size": 0}},
        {"mimeType": "x/y", "filename": "rotto.bin", "body": {"data": "!!!"}},
        {"mimeType": "x/y", "filename": "  a\nb\u{0}.pdf  ", "body": {"attachmentId": "A1"}},
        {"mimeType": "x/y", "filename": "=?utf-8?B?Y2FmZsOvLnBkZg==?=", "body": {"attachmentId": "A2", "size": 9}},
        {"mimeType": "x/y", "filename": "   ", "body": {"attachmentId": "A3"}}
    ]});
    let got = collect(&payload);
    let names: Vec<&str> = got.attachments.iter().map(|a| a.filename.as_str()).collect();
    assert_eq!(names, vec!["a b .pdf", "caffï.pdf"]);
    assert_eq!(got.attachments[0].size, 0);
}

#[test]
fn charsets_are_honoured() {
    let mut part = text_part("text/plain", "");
    part["headers"] = json!([{"name": "Content-Type", "value": "text/plain; charset=ISO-8859-1"}]);
    part["body"]["data"] = json!(URL_SAFE_NO_PAD.encode([0x63u8, 0x61, 0x66, 0x66, 0xE8]));
    assert_eq!(collect(&part).plain.as_deref(), Some("caffè"));
    part["headers"] = json!([{"name": "Content-Type", "value": "text/plain; format=flowed; charset=shift_jis"}]);
    part["body"]["data"] = json!(b64("è"));
    assert_eq!(collect(&part).plain.as_deref(), Some("è"));
    part["headers"] = json!([]);
    assert_eq!(collect(&part).plain.as_deref(), Some("è"));
}

#[test]
fn invalid_base64_leaves_the_text_empty() {
    let payload = json!({"mimeType": "text/plain", "body": {"data": "!!!not base64"}});
    assert!(collect(&payload).plain.is_none());
}

#[test]
fn weird_shapes_do_not_panic() {
    for payload in [
        json!(null),
        json!([]),
        json!("x"),
        json!({}),
        json!({"mimeType": 5, "parts": "x", "body": []}),
        json!({"mimeType": "text/plain", "body": {"data": 5}}),
        json!({"parts": [null, 1, "a", {"parts": {}}]}),
        json!({"filename": 7, "headers": "x"}),
        json!({"mimeType": "text/plain", "headers": [{"name": 1, "value": 2}, null], "body": {"data": b64("ok")}}),
    ] {
        let _ = collect(&payload);
    }
}

#[test]
fn depth_is_capped() {
    let mut node = text_part("text/plain", "fondo");
    for _ in 0..(DEPTH_MAX + 5) {
        node = json!({"mimeType": "multipart/mixed", "parts": [node]});
    }
    assert!(collect(&node).plain.is_none());
    let mut shallow = text_part("text/plain", "fondo");
    for _ in 0..DEPTH_MAX {
        shallow = json!({"mimeType": "multipart/mixed", "parts": [shallow]});
    }
    assert_eq!(collect(&shallow).plain.as_deref(), Some("fondo"));
}

#[test]
fn the_part_count_is_capped() {
    let mut parts: Vec<Value> = (0..PARTS_MAX).map(|_| json!({"mimeType": "x/y"})).collect();
    parts.push(text_part("text/plain", "troppo tardi"));
    let payload = json!({"mimeType": "multipart/mixed", "parts": parts});
    assert!(collect(&payload).plain.is_none());
}

#[test]
fn the_attachment_count_is_capped() {
    let parts: Vec<Value> = (0..200)
        .map(|n| json!({"mimeType": "x/y", "filename": format!("f{n}"), "body": {"attachmentId": format!("a{n}")}}))
        .collect();
    let got = collect(&json!({"mimeType": "multipart/mixed", "parts": parts}));
    assert_eq!(got.attachments.len(), ATTACHMENTS_MAX);
}

#[test]
fn an_oversized_text_part_is_read_in_part() {
    let big = "a".repeat(TEXT_B64_MAX + 10);
    let payload = json!({"mimeType": "text/plain", "body": {"data": big}});
    assert_eq!(collect(&payload).plain.map(|t| t.chars().count()), Some(TEXT_B64_MAX / 4 * 3));
}

#[test]
fn headers_are_found_case_insensitively() {
    let node = json!({"headers": [{"name": "SUBJECT", "value": "x"}, {"name": "Subject", "value": "y"}]});
    assert_eq!(header_value(&node, "subject"), Some("x"));
    assert_eq!(header_value(&node, "From"), None);
    assert_eq!(header_value(&json!({}), "From"), None);
}

#[test]
fn charset_parameters_are_parsed() {
    assert_eq!(charset_of("text/plain; charset=utf-8"), "utf-8");
    assert_eq!(charset_of("text/plain; CHARSET=\"ISO-8859-1\"; format=flowed"), "ISO-8859-1");
    assert_eq!(charset_of("text/plain"), "");
    assert_eq!(charset_of("text/plain; charset="), "");
}
