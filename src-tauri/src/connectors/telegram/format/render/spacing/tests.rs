use super::*;

#[test]
fn a_blank_line_is_found_only_between_the_two_blocks() {
    assert!(blank_line_between("a\n\nb", 1, 3));
    assert!(blank_line_between("a\n  \t\nb", 1, 6));
    assert!(blank_line_between("a\r\n\r\nb", 1, 5));
    assert!(!blank_line_between("a\nb", 1, 2));
    assert!(!blank_line_between("a\n  b", 1, 4));
    assert!(!blank_line_between("ab", 1, 1));
}

#[test]
fn a_quote_marker_line_counts_as_blank() {
    assert!(blank_line_between("a\n> \n> b", 1, 7));
    assert!(blank_line_between("a\n>>\n>> b", 1, 8));
    assert!(!blank_line_between("a\n> b", 1, 4));
}

#[test]
fn out_of_range_positions_are_never_blank() {
    assert!(!blank_line_between("a", 5, 9));
    assert!(!blank_line_between("è\n\nb", 1, 4));
}

#[test]
fn the_content_end_drops_trailing_line_breaks() {
    assert_eq!(content_end("---\n\nx", 0..4), 3);
    assert_eq!(content_end("ab\r\n", 0..4), 2);
    assert_eq!(content_end("ab", 0..2), 2);
    assert_eq!(content_end("ab", 1..9), 1);
}
