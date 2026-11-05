use super::*;
use serde_json::json;

#[test]
fn valid_file_paths() {
    for path in ["photos/file_1.jpg", "documents/file_12.pdf", "a", "a-b/c.d/e_f", "x..y", ".hidden"] {
        assert!(valid_file_path(path), "{path:?}");
    }
    assert!(valid_file_path(&"a".repeat(256)));
}

#[test]
fn invalid_file_paths() {
    let too_long = "a".repeat(257);
    for path in [
        "",
        "/etc/passwd",
        "../x",
        "a/../b",
        "a/..",
        "..",
        "a//b",
        "a/",
        "a b",
        "a?x=1",
        "a#b",
        "a%2e%2e/b",
        "a\\b",
        "è.jpg",
        "a\nb",
        "a:b",
        "a@b",
        too_long.as_str(),
    ] {
        assert!(!valid_file_path(path), "{path:?}");
    }
}

#[test]
fn file_info_needs_a_valid_path() {
    let info = parse_file_info(&json!({"file_id": "x", "file_path": "photos/f.jpg", "file_size": 1234})).unwrap();
    assert_eq!(info, FileInfo { path: "photos/f.jpg".to_string(), size: Some(1234) });
    let no_size = parse_file_info(&json!({"file_path": "photos/f.jpg"})).unwrap();
    assert_eq!(no_size.size, None);
    for bad in [
        json!({}),
        json!(null),
        json!([]),
        json!({"file_path": 5}),
        json!({"file_path": "../x"}),
        json!({"file_path": ""}),
    ] {
        assert!(matches!(parse_file_info(&bad), Err(ConnectorError::Transient(_))), "{bad}");
    }
}

#[test]
fn file_info_rejects_files_over_the_hub_limit() {
    let at_limit = json!({"file_path": "a/b", "file_size": ATTACHMENT_SIZE_MAX});
    assert!(parse_file_info(&at_limit).is_ok());
    let over = json!({"file_path": "a/b", "file_size": ATTACHMENT_SIZE_MAX + 1});
    assert!(matches!(parse_file_info(&over), Err(ConnectorError::Transient(_))));
    let odd = json!({"file_path": "a/b", "file_size": "grande"});
    assert_eq!(parse_file_info(&odd).unwrap().size, None);
}

#[test]
fn only_the_largest_photo_is_offered() {
    let message = json!({"photo": [
        {"file_id": "small", "file_size": 100, "width": 90, "height": 90},
        {"file_id": "big", "file_size": 900, "width": 800, "height": 800},
        {"file_id": "mid", "file_size": 400, "width": 320, "height": 320}
    ]});
    let files = message_files(&message, 7);
    assert_eq!(files, vec![RemoteFile { id: "big".to_string(), name: "foto-7.jpg".to_string(), size: Some(900) }]);
}

#[test]
fn photo_sizes_without_weight_are_compared_by_area() {
    let message = json!({"photo": [
        {"file_id": "a", "width": 100, "height": 100},
        {"file_id": "b", "width": 1000, "height": 500},
        {"file_id": "c", "width": 200, "height": 200}
    ]});
    assert_eq!(message_files(&message, 1)[0].id, "b");
}

#[test]
fn a_document_keeps_its_name_and_size() {
    let message = json!({"document": {"file_id": "doc1", "file_name": "relazione.pdf", "file_size": 2048}});
    assert_eq!(
        message_files(&message, 3),
        vec![RemoteFile { id: "doc1".to_string(), name: "relazione.pdf".to_string(), size: Some(2048) }]
    );
}

#[test]
fn a_document_without_a_name_gets_one() {
    let message = json!({"document": {"file_id": "doc1"}});
    let files = message_files(&message, 3);
    assert_eq!(files[0].name, "documento-3");
    assert_eq!(files[0].size, None);
    let blank = json!({"document": {"file_id": "doc1", "file_name": "  "}});
    assert_eq!(message_files(&blank, 4)[0].name, "documento-4");
}

#[test]
fn a_photo_with_a_document_gives_both() {
    let message = json!({"photo": [{"file_id": "p"}], "document": {"file_id": "d"}});
    assert_eq!(message_files(&message, 1).len(), 2);
}

#[test]
fn hostile_attachments_give_nothing() {
    let hostile = [
        json!({}),
        json!({"photo": null}),
        json!({"photo": "x"}),
        json!({"photo": []}),
        json!({"photo": [null, 5, "x", [], {}]}),
        json!({"photo": [{"file_id": 5}]}),
        json!({"photo": [{"file_id": ""}]}),
        json!({"document": null}),
        json!({"document": []}),
        json!({"document": {"file_id": null}}),
        json!({"document": "x"}),
    ];
    for message in hostile {
        assert!(message_files(&message, 1).is_empty(), "{message}");
    }
    let huge = json!({"photo": [{"file_id": "a", "width": u64::MAX, "height": u64::MAX, "file_size": u64::MAX}]});
    assert_eq!(message_files(&huge, 1).len(), 1);
}

#[test]
fn a_very_long_document_name_is_capped() {
    let message = json!({"document": {"file_id": "d", "file_name": "é".repeat(1000)}});
    assert_eq!(message_files(&message, 1)[0].name.chars().count(), 255);
}
