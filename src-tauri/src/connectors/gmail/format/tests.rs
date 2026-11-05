use super::*;

#[test]
fn the_sender_is_name_and_address() {
    assert_eq!(sender_name("Mario Rossi <mario@gmail.com>"), "Mario Rossi <mario@gmail.com>");
    assert_eq!(sender_name("\"Rossi, Mario\" <mario@gmail.com>"), "Rossi, Mario <mario@gmail.com>");
    assert_eq!(sender_name("  Mario   Rossi \t <mario@gmail.com> "), "Mario Rossi <mario@gmail.com>");
}

#[test]
fn without_a_name_only_the_address_is_shown() {
    assert_eq!(sender_name("<mario@gmail.com>"), "mario@gmail.com");
    assert_eq!(sender_name("mario@gmail.com"), "mario@gmail.com");
    assert_eq!(sender_name("\"\" <mario@gmail.com>"), "mario@gmail.com");
}

#[test]
fn an_unknown_sender_has_a_fixed_label() {
    for from in ["", "   ", "<>", "\"\" <>", "\n\t"] {
        assert_eq!(sender_name(from), "Mittente sconosciuto", "{from:?}");
    }
}

#[test]
fn a_name_without_an_address_is_kept() {
    assert_eq!(sender_name("Mario <>"), "Mario");
}

#[test]
fn the_sender_is_one_clean_line_of_at_most_120_chars() {
    let hostile = "Ma\nrio\u{0}\u{202e} Rossi <m@x.it>";
    assert_eq!(sender_name(hostile), "Ma rio Rossi <m@x.it>");
    let long = sender_name(&format!("{} <m@x.it>", "😀".repeat(500)));
    assert_eq!(long.chars().count(), TITLE_MAX);
    assert!(!long.contains('\n'));
    assert_eq!(sender_name(&"a".repeat(1000)).chars().count(), ADDRESS_MAX.min(TITLE_MAX));
}

#[test]
fn a_stray_angle_bracket_does_not_panic() {
    for from in ["<", ">", "a<", "<a", "a>b", "è<è>è", "<<<>>>", "x <y <z>"] {
        let _ = sender_name(from);
    }
    assert_eq!(sender_name("x <y <z>"), "x <y <z>");
}

#[test]
fn the_title_is_a_single_line_of_at_most_120_chars() {
    assert_eq!(thread_title("Riunione  di\nlunedì"), "Riunione di lunedì");
    assert_eq!(thread_title(&"é".repeat(500)).chars().count(), TITLE_MAX);
    assert_eq!(thread_title(&"é".repeat(120)).chars().count(), 120);
}

#[test]
fn an_empty_title_has_a_placeholder() {
    for subject in ["", "  ", "\n\t", "\u{200b}"] {
        assert_eq!(thread_title(subject), "(senza oggetto)");
    }
}

#[test]
fn sizes_use_the_italian_decimal_comma() {
    assert_eq!(size_text(0), "0 B");
    assert_eq!(size_text(1023), "1023 B");
    assert_eq!(size_text(1024), "1 KB");
    assert_eq!(size_text(3 * 1024 + 512), "3,5 KB");
    assert_eq!(size_text(40 * 1024 * 1024), "40 MB");
    assert_eq!(size_text(12 * 1024 * 1024 + 350_000), "12,3 MB");
    assert!(size_text(u64::MAX).ends_with("MB"));
}

#[test]
fn the_body_has_header_blank_line_and_text() {
    let body = email_body("Ciao", "Mario <m@x.it>", "Tue, 14 Nov 2023", &[], "Testo\ndel messaggio");
    assert_eq!(body, "Oggetto: Ciao\nDa: Mario <m@x.it>\nData: Tue, 14 Nov 2023\n\nTesto\ndel messaggio");
}

#[test]
fn not_imported_attachments_sit_in_the_header() {
    let skipped = vec![("a.pdf".to_string(), 12 * 1024 * 1024 + 350_000), ("b.zip".to_string(), 40 * 1024 * 1024)];
    let body = email_body("", "m@x.it", "oggi", &skipped, "t");
    assert_eq!(
        body,
        "Oggetto: (senza oggetto)\nDa: m@x.it\nData: oggi\nAllegati non importati: a.pdf (12,3 MB), b.zip (40 MB)\n\nt"
    );
}

#[test]
fn a_long_list_of_skipped_files_is_summarised() {
    let skipped: Vec<(String, u64)> = (0..15).map(|n| (format!("f{n}"), 1024)).collect();
    let body = email_body("s", "m", "d", &skipped, "t");
    assert!(body.contains("f9 (1 KB), e altri 5\n\n"), "{body}");
}

#[test]
fn the_body_header_is_always_single_line() {
    let skipped = vec![("a\nb.pdf".to_string(), 1)];
    let body = email_body("og\ngetto", "da\nme", "da\nta", &skipped, "t");
    let (header, _) = body.split_once("\n\n").unwrap();
    assert_eq!(header.lines().count(), 4);
    assert!(header.contains("a b.pdf (1 B)"));
}

#[test]
fn the_date_is_the_header_or_internal_date_in_utc() {
    assert_eq!(date_label("  Tue, 14 Nov 2023\n22:13:20 +0000 ", 0), "Tue, 14 Nov 2023 22:13:20 +0000");
    assert_eq!(date_label("", 1_700_000_000_000), "14/11/2023 22:13 UTC");
    assert_eq!(date_label("\n", 0), "01/01/1970 00:00 UTC");
    assert_eq!(date_label(&"x".repeat(500), 0).chars().count(), DATE_MAX);
    assert_eq!(date_label("", i64::MAX), "data sconosciuta");
}
