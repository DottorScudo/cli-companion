use super::*;

#[test]
fn a_saved_offset_round_trips() {
    assert_eq!(parse(Some(&to_value(120))), Some(120));
    assert_eq!(parse(Some(&to_value(0))), Some(0));
    assert_eq!(to_value(5), json!({"offset": 5}));
}

#[test]
fn a_missing_or_malformed_cursor_means_start_from_now() {
    assert_eq!(parse(None), None);
    let malformed = [
        json!(null),
        json!([]),
        json!("5"),
        json!(5),
        json!({}),
        json!({"offset": "5"}),
        json!({"offset": 1.5}),
        json!({"offset": null}),
        json!({"offset": u64::MAX}),
    ];
    for bad in malformed {
        assert_eq!(parse(Some(&bad)), None, "{bad}");
    }
}
