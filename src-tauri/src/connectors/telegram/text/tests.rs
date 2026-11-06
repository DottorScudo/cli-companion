use super::*;

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Le parti come testi, con il rimando sull'ultima se il testo e' troncato.
fn split(text: &str) -> Vec<String> {
    let layout = layout(text);
    let last = layout.parts.len().saturating_sub(1);
    let mut parts: Vec<String> = layout.parts.iter().map(|range| text[range.clone()].to_string()).collect();
    if layout.truncated {
        parts[last].push_str(TRUNCATED_MARKER);
    }
    parts
}

#[test]
fn empty_or_blank_text_has_no_parts() {
    for text in ["", " ", "\n\n", " \t\n "] {
        assert!(split(text).is_empty(), "{text:?}");
    }
}

#[test]
fn short_text_is_one_part() {
    assert_eq!(split("ciao"), vec!["ciao".to_string()]);
    assert_eq!(split("  ciao\n"), vec!["ciao".to_string()]);
    let exact = "a".repeat(PART_UNITS_MAX);
    assert_eq!(split(&exact), vec![exact]);
}

#[test]
fn text_over_the_limit_is_cut_at_the_last_newline() {
    let text = format!("{}\n{}", "a".repeat(100), "b".repeat(PART_UNITS_MAX));
    let parts = split(&text);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "a".repeat(100));
    assert_eq!(parts[1], "b".repeat(PART_UNITS_MAX));
}

#[test]
fn a_newline_wins_over_a_later_space() {
    let text = format!("{}\n{} {}", "a".repeat(50), "b".repeat(50), "c".repeat(PART_UNITS_MAX));
    let parts = split(&text);
    assert_eq!(parts[0], "a".repeat(50));
}

#[test]
fn without_newlines_it_cuts_at_the_last_space() {
    let text = format!("{} {}", "a".repeat(PART_UNITS_MAX - 10), "b".repeat(100));
    let parts = split(&text);
    assert_eq!(parts, vec!["a".repeat(PART_UNITS_MAX - 10), "b".repeat(100)]);
}

#[test]
fn without_separators_it_cuts_hard_at_the_limit() {
    let parts = split(&"a".repeat(PART_UNITS_MAX + 5));
    assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![PART_UNITS_MAX, 5]);
}

#[test]
fn the_limit_is_in_utf16_units_and_never_cuts_a_character() {
    // Ogni emoji vale 2 unita' UTF-16 (4 byte).
    let text = "😀".repeat(PART_UNITS_MAX);
    let parts = split(&text);
    assert_eq!(parts.len(), 2);
    assert!(
        parts.iter().all(|p| units(p) <= PART_UNITS_MAX),
        "{:?}",
        parts.iter().map(|p| units(p)).collect::<Vec<_>>()
    );
    assert_eq!(parts[0].chars().count(), PART_UNITS_MAX / 2);
    assert_eq!(parts.concat(), text);
}

#[test]
fn multibyte_text_is_cut_on_a_char_boundary() {
    let text = "è".repeat(PART_UNITS_MAX + 1);
    let parts = split(&text);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts.concat(), text);
}

#[test]
fn the_text_is_capped_to_three_parts_with_a_marker() {
    let text = "parola ".repeat(5000);
    let parts = split(&text);
    assert_eq!(parts.len(), PARTS_MAX);
    assert!(parts.iter().all(|p| units(p) <= PART_UNITS_MAX));
    let last = parts.last().unwrap();
    assert!(last.ends_with("\n… (testo completo nell'HUB)"));
}

#[test]
fn the_marker_fits_even_with_wide_characters() {
    let parts = split(&"😀".repeat(PART_UNITS_MAX * 3));
    assert_eq!(parts.len(), PARTS_MAX);
    assert!(parts.iter().all(|p| units(p) <= PART_UNITS_MAX));
    assert!(parts[PARTS_MAX - 1].ends_with("nell'HUB)"));
}

#[test]
fn a_text_that_exactly_fills_three_parts_has_no_marker() {
    let text = format!("{}\n{}\n{}", "a".repeat(PART_UNITS_MAX - 1), "b".repeat(PART_UNITS_MAX - 1), "c".repeat(100));
    let parts = split(&text);
    assert_eq!(parts.len(), 3);
    assert!(!parts[2].contains("HUB"));
}

#[test]
fn a_cut_point_is_never_zero() {
    assert!(cut_point("😀😀", 1) > 0);
    assert_eq!(cut_point("abc", 10), 3);
}

#[test]
fn the_parts_are_ranges_of_the_text_without_surrounding_spaces() {
    let text = format!("  {}\n\n{}  ", "a".repeat(PART_UNITS_MAX), "b".repeat(10));
    let found = layout(&text);
    assert_eq!(found.parts, vec![2..2 + PART_UNITS_MAX, PART_UNITS_MAX + 4..PART_UNITS_MAX + 14]);
    assert!(!found.truncated);
}

#[test]
fn only_a_text_longer_than_three_parts_is_truncated() {
    assert!(layout(&"parola ".repeat(5000)).truncated);
    assert!(!layout(&"parola ".repeat(100)).truncated);
    assert_eq!(layout(""), Layout { parts: Vec::new(), truncated: false });
}
