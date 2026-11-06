use super::*;
use crate::board::SUMMARY_MAX;

/// Il testo di una parte senza la sua etichetta "(parte i/n)".
fn content(part: &str, index: usize, count: usize) -> &str {
    part.strip_prefix(&format!("(parte {index}/{count})\n"))
        .unwrap_or_else(|| panic!("etichetta mancante: {part:.40?}"))
}

/// I testi delle parti, senza etichette, controllando etichette e limiti.
fn contents(parts: &[String]) -> Vec<&str> {
    let count = parts.len();
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            assert!(part.chars().count() <= BODY_MAX, "parte {} di {} caratteri", i + 1, part.chars().count());
            content(part, i + 1, count)
        })
        .collect()
}

/// `count` paragrafi di parole separati da una riga vuota.
fn paragraphs(count: usize) -> String {
    (0..count).map(|i| format!("Paragrafo {i}: {}", "parola ".repeat(60).trim_end())).collect::<Vec<_>>().join("\n\n")
}

#[test]
fn a_text_that_fits_one_message_is_one_unchanged_part() {
    assert!(split("", Remainder::Lost).is_empty());
    assert_eq!(split("ciao", Remainder::Lost), ["ciao"]);
    let exact = "é".repeat(BODY_MAX);
    assert_eq!(split(&exact, Remainder::Lost), [exact]);
}

#[test]
fn a_long_text_is_cut_between_paragraphs_and_rebuilds_exactly() {
    let text = paragraphs(50);
    assert!(text.chars().count() > 2 * BODY_MAX);
    let parts = split(&text, Remainder::Lost);
    assert_eq!(parts.len(), 3);
    let contents = contents(&parts);
    assert_eq!(contents.concat(), text);
    for next in &contents[1..] {
        assert!(next.starts_with("\n\nParagrafo"), "{next:.30?}");
    }
}

#[test]
fn without_paragraphs_the_cut_falls_between_lines() {
    let text = (0..400).map(|i| format!("riga {i} {}", "x".repeat(40))).collect::<Vec<_>>().join("\n");
    let parts = split(&text, Remainder::Lost);
    let contents = contents(&parts);
    assert_eq!(contents.concat(), text);
    assert!(contents[1].starts_with("\nriga ") && !contents[1].starts_with("\n\n"), "{:.30?}", contents[1]);
}

#[test]
fn without_lines_the_cut_falls_between_words() {
    let text = "parola ".repeat(3000).trim_end().to_string();
    let parts = split(&text, Remainder::Lost);
    let contents = contents(&parts);
    assert_eq!(contents.concat(), text);
    assert!(contents[0].ends_with("parola") && contents[1].starts_with(" parola"));
}

#[test]
fn a_text_without_spaces_is_cut_between_characters_never_inside_one() {
    let text = "é€😀".repeat(4000);
    let parts = split(&text, Remainder::Lost);
    assert_eq!(parts.len(), 2);
    assert_eq!(contents(&parts).concat(), text);
}

#[test]
fn a_cut_outside_a_code_block_is_preferred() {
    let prose = "parola ".repeat(1000);
    let code = (0..100).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
    let text = format!("{}\n\n```rust\n{code}\n```\n\n{}", prose.trim_end(), prose.trim_end());
    let parts = split(&text, Remainder::Lost);
    let contents = contents(&parts);
    assert_eq!(contents.concat(), text);
    assert!(!contents[0].contains("```"), "il blocco intero passa alla parte successiva");
}

#[test]
fn a_code_block_cut_inside_is_closed_and_reopened() {
    let code = (0..1500).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
    let text = format!("Il codice:\n\n```rust\n{code}\n```\nFine.");
    let parts = split(&text, Remainder::Lost);
    let contents = contents(&parts);
    assert!(parts.len() >= 3);
    assert!(contents[0].ends_with(";\n```"), "{:?}", &contents[0][contents[0].len() - 20..]);
    assert!(contents[1].starts_with("```rust\nlet x"), "{:.30?}", contents[1]);
    let last = contents.last().unwrap();
    assert!(last.starts_with("```rust\n") && last.ends_with("\n```\nFine."));
    let code_lines = |c: &str| c.lines().filter(|l| l.starts_with("let x")).count();
    assert_eq!(contents.iter().map(|c| code_lines(c)).sum::<usize>(), 1500, "nessuna riga persa");
}

#[test]
fn an_unclosed_code_block_is_still_reopened() {
    let text = format!("```\n{}", "riga di codice\n".repeat(1000).trim_end());
    let contents_of = split(&text, Remainder::Lost);
    let contents = contents(&contents_of);
    assert!(contents[0].ends_with("\n```") && contents[1].starts_with("```\n"));
}

#[test]
fn a_too_long_fence_is_not_treated_as_a_code_block() {
    let fence = "`".repeat(FENCE_MARKER_MAX + 1);
    let text = format!("{fence}\n{}", "riga\n".repeat(3000).trim_end());
    let parts = split(&text, Remainder::Lost);
    assert_eq!(contents(&parts).concat(), text);
}

#[test]
fn the_longest_guaranteed_text_fits_and_covers_a_plan_report() {
    const { assert!(LONG_BODY_MAX >= SUMMARY_MAX) };
    // Parole lunghe quasi quanto la finestra: il taglio cade il prima possibile.
    let word = "x".repeat(BOUNDARY_WINDOW - 1);
    let mut text =
        std::iter::repeat_n(word.as_str(), LONG_BODY_MAX / BOUNDARY_WINDOW + 1).collect::<Vec<_>>().join(" ");
    text.truncate(LONG_BODY_MAX);
    let parts = split(text.trim_end(), Remainder::Lost);
    assert!(parts.len() <= MAX_PARTS);
    assert_eq!(contents(&parts).concat(), text.trim_end());
}

#[test]
fn beyond_the_last_part_a_notice_says_how_much_is_missing() {
    let text = paragraphs(200);
    let total = text.chars().count();
    for (remainder, place) in [(Remainder::Lost, "oltre il limite di 8 messaggi"), (Remainder::InTask, "Tasks Manager")]
    {
        let parts = split(&text, remainder);
        assert_eq!(parts.len(), MAX_PARTS);
        let contents = contents(&parts);
        let (last, notice) = contents[MAX_PARTS - 1].rsplit_once("\n\n").unwrap();
        assert!(notice.contains(place), "{notice}");
        let kept: usize =
            contents[..MAX_PARTS - 1].iter().map(|c| c.chars().count()).sum::<usize>() + last.chars().count();
        assert!(notice.contains(&format!("mancano {} caratteri", total - kept)), "{notice}");
        assert!(text.starts_with(&format!("{}{last}", contents[..MAX_PARTS - 1].concat())));
    }
}

#[test]
fn the_notices_fit_their_reserve() {
    for remainder in [Remainder::Lost, Remainder::InTask] {
        assert!(remainder.notice(usize::MAX).chars().count() + 2 <= NOTICE_RESERVE);
    }
}
