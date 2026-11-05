use super::*;
use serde_json::json;

#[test]
fn a_normal_page_is_parsed() {
    let body = json!({
        "messages": [{"id": "18c1", "threadId": "t1"}, {"id": "18c2", "threadId": "t1"}],
        "nextPageToken": "09876_tok",
        "resultSizeEstimate": 2
    });
    let page = parse_list(&body).unwrap();
    assert_eq!(page.ids, vec!["18c1", "18c2"]);
    assert_eq!(page.next_page_token.as_deref(), Some("09876_tok"));
}

#[test]
fn no_messages_means_an_empty_page() {
    for body in [json!({}), json!({"resultSizeEstimate": 0}), json!({"messages": null}), json!({"messages": []})] {
        let page = parse_list(&body).unwrap();
        assert!(page.ids.is_empty());
        assert!(page.next_page_token.is_none());
    }
}

#[test]
fn invalid_ids_are_skipped() {
    let body = json!({"messages": [
        {"id": "ok1"}, {"id": "../x"}, {"id": ""}, {"id": 5}, {"threadId": "t"}, "str", null, {"id": "a b"}, {"id": "ok2"}
    ]});
    assert_eq!(parse_list(&body).unwrap().ids, vec!["ok1", "ok2"]);
}

#[test]
fn the_ids_are_capped() {
    let messages: Vec<Value> = (0..500).map(|n| json!({"id": format!("m{n}")})).collect();
    let page = parse_list(&json!({"messages": messages})).unwrap();
    assert_eq!(page.ids.len(), IDS_MAX);
    assert_eq!(page.ids[0], "m0");
}

#[test]
fn the_page_token_is_validated() {
    for (token, valid) in [
        (json!("abc"), true),
        (json!("a".repeat(1024)), true),
        (json!("a".repeat(1025)), false),
        (json!(""), false),
        (json!("a b"), false),
        (json!("caffè"), false),
        (json!(5), false),
        (json!(null), false),
    ] {
        let page = parse_list(&json!({"nextPageToken": token})).unwrap();
        assert_eq!(page.next_page_token.is_some(), valid, "{token}");
    }
}

#[test]
fn a_body_of_the_wrong_shape_is_an_error() {
    for body in [json!(null), json!([]), json!("x"), json!(3), json!({"messages": "x"}), json!({"messages": {}})] {
        assert!(matches!(parse_list(&body), Err(ConnectorError::Transient(_))), "{body}");
    }
}
