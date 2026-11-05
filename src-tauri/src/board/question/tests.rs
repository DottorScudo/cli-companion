use super::*;
use serde_json::json;

#[test]
fn a_question_is_trimmed_and_options_are_optional() {
    assert_eq!(parse_question(&json!({ "question": "  Which db?  " })), Ok(("Which db?".to_string(), vec![])));
    assert_eq!(parse_question(&json!({ "question": "q", "options": null })), Ok(("q".to_string(), vec![])));
    assert_eq!(parse_question(&json!({ "question": "q", "options": [] })), Ok(("q".to_string(), vec![])));
}

#[test]
fn the_question_is_required_and_bounded() {
    assert_eq!(parse_question(&json!({})).unwrap_err(), "question is missing");
    assert_eq!(parse_question(&json!({ "question": null })).unwrap_err(), "question is missing");
    assert_eq!(parse_question(&json!({ "question": "   " })).unwrap_err(), "question is missing");
    assert_eq!(parse_question(&json!({ "question": 3 })).unwrap_err(), "question must be a string");
    assert!(parse_question(&json!({ "question": "x".repeat(QUESTION_MAX) })).is_ok());
    let too_long = parse_question(&json!({ "question": "x".repeat(QUESTION_MAX + 1) }));
    assert!(too_long.unwrap_err().contains("too long"));
    // Il limite e' in caratteri, non in byte.
    assert!(parse_question(&json!({ "question": "è".repeat(QUESTION_MAX) })).is_ok());
}

#[test]
fn options_are_trimmed_and_kept_in_order() {
    let (_, options) = parse_question(&json!({ "question": "q", "options": [" B ", "A"] })).unwrap();
    assert_eq!(options, ["B", "A"]);
}

#[test]
fn options_are_bounded() {
    let six: Vec<String> = (0..=OPTIONS_MAX).map(|i| format!("o{i}")).collect();
    assert!(parse_question(&json!({ "question": "q", "options": six })).unwrap_err().contains("too many"));
    let five: Vec<String> = (0..OPTIONS_MAX).map(|i| format!("o{i}")).collect();
    assert!(parse_question(&json!({ "question": "q", "options": five })).is_ok());
    let long = "x".repeat(OPTION_MAX + 1);
    assert!(parse_question(&json!({ "question": "q", "options": [long] })).unwrap_err().contains("too long"));
    assert!(parse_question(&json!({ "question": "q", "options": [""] })).is_err());
}

#[test]
fn options_are_single_line_unique_strings() {
    let multiline = parse_question(&json!({ "question": "q", "options": ["a\nb"] }));
    assert_eq!(multiline.unwrap_err(), "an option must be a single line");
    let duplicate = parse_question(&json!({ "question": "q", "options": ["a", " a "] }));
    assert_eq!(duplicate.unwrap_err(), "duplicate option: a");
    let not_text = parse_question(&json!({ "question": "q", "options": [1] }));
    assert_eq!(not_text.unwrap_err(), "options must be an array of strings");
    let not_array = parse_question(&json!({ "question": "q", "options": "a" }));
    assert_eq!(not_array.unwrap_err(), "options must be an array of strings");
}
