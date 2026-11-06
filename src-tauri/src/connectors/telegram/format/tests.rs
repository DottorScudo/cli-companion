use super::*;
use crate::connectors::telegram::text::{PARTS_MAX, PART_UNITS_MAX};

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Il testo visibile di un HTML di Telegram, controllando che sia ben formato: ogni tag
/// chiuso nell'ordine giusto, solo entita' note e intere. In panico se non lo e'.
fn visible_of(html: &str) -> String {
    let mut out = String::new();
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        match c {
            '<' => {
                let end = rest.find('>').unwrap_or_else(|| panic!("tag spezzato: {rest:?}"));
                let tag = &rest[1..end];
                match tag.strip_prefix('/') {
                    Some(name) => assert_eq!(open.pop().as_deref(), Some(name), "chiusura fuori ordine in {html:?}"),
                    None => open.push(tag.split(' ').next().unwrap().to_string()),
                }
                rest = &rest[end + 1..];
            }
            '&' => {
                let end = rest.find(';').unwrap_or_else(|| panic!("entita' spezzata: {rest:?}"));
                out.push(match &rest[..=end] {
                    "&amp;" => '&',
                    "&lt;" => '<',
                    "&gt;" => '>',
                    "&quot;" => '"',
                    other => panic!("entita' sconosciuta {other:?}"),
                });
                rest = &rest[end + 1..];
            }
            '>' => panic!("'>' non escapato in {html:?}"),
            _ => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    assert!(open.is_empty(), "tag non chiusi {open:?} in {html:?}");
    out
}

fn assert_well_formed(parts: &[Part]) {
    assert!(parts.len() <= PARTS_MAX);
    for part in parts {
        assert_eq!(visible_of(&part.html), part.plain);
        assert!(units(&part.plain) <= PART_UNITS_MAX, "{} unita'", units(&part.plain));
        assert!(!part.plain.trim().is_empty());
    }
}

#[test]
fn an_empty_or_blank_text_has_no_parts() {
    for text in ["", "   ", "\n\n\t", "```\n```"] {
        assert!(parts(text).is_empty(), "{text:?}");
    }
}

#[test]
fn a_short_text_is_one_html_part_with_its_plain_text() {
    let found = parts("# Piano\n\n- **uno** & due\n- `tre`");
    assert_eq!(
        found,
        vec![Part {
            html: "<b>Piano</b>\n\n• <b>uno</b> &amp; due\n• <code>tre</code>".to_string(),
            plain: "Piano\n\n• uno & due\n• tre".to_string(),
        }]
    );
}

#[test]
fn a_plain_text_part_is_the_text_itself() {
    let text = "Comandi:\n/use <n>: scegli il thread\n\n1. ⏳ Lavoro › Piano";
    let found = parts(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].plain, text);
    assert_eq!(found[0].html, text.replace('<', "&lt;").replace('>', "&gt;"));
}

/// Un grassetto lungo a cavallo del primo taglio, poi un blocco di codice a cavallo del
/// secondo, e piu' testo di quanto stia in tre parti.
fn long_markdown() -> String {
    format!("**{}fine**\n\n```rust\n{}```\n\ncoda", "parola ".repeat(1000), "let x = 1 < 2 && y;\n".repeat(400))
}

#[test]
fn tags_cut_by_a_part_are_closed_and_reopened_with_the_same_attributes() {
    let found = parts(&long_markdown());
    assert_eq!(found.len(), PARTS_MAX);
    assert_well_formed(&found);
    assert!(found[0].html.starts_with("<b>parola") && found[0].html.ends_with("</b>"), "{}", &found[0].html[..20]);
    assert!(found[1].html.starts_with("<b>parola"));
    assert!(found[1].html.contains("fine</b>\n\n<pre><code class=\"language-rust\">let x = 1 &lt; 2 &amp;&amp; y;"));
    assert!(found[1].html.ends_with("</code></pre>"));
    assert!(found[2].html.starts_with("<pre><code class=\"language-rust\">let x"));
}

#[test]
fn parts_are_cut_at_the_end_of_a_line_or_word_never_inside_an_entity() {
    let found = parts(&long_markdown());
    assert!(found[0].plain.ends_with("parola"));
    assert!(found[1].plain.ends_with("&& y;"));
    assert!(found[2].plain.starts_with("let x"));
}

#[test]
fn a_truncated_text_ends_with_the_marker_outside_every_tag() {
    let found = parts(&long_markdown());
    let last = found.last().unwrap();
    assert!(
        last.html.ends_with("</code></pre>\n… (testo completo nell'HUB)"),
        "{}",
        &last.html[last.html.len() - 80..]
    );
    assert!(last.plain.ends_with("\n… (testo completo nell'HUB)"));
}

#[test]
fn the_limit_counts_the_visible_text_not_tags_or_entities() {
    // 4096 «&» stanno in un messaggio anche se l'HTML e' lungo cinque volte tanto.
    let found = parts(&"&".repeat(PART_UNITS_MAX));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].html.len(), PART_UNITS_MAX * "&amp;".len());
    let bold = parts(&format!("**{}**", "a".repeat(PART_UNITS_MAX)));
    assert_eq!(bold.len(), 1);
    let over = parts(&format!("**{}**", "a".repeat(PART_UNITS_MAX + 1)));
    assert_eq!(over.len(), 2);
    assert_well_formed(&over);
    assert_eq!(over[1].html, "<b>a</b>");
}

#[test]
fn wide_characters_are_counted_in_utf16_units() {
    let found = parts(&format!("*{}*", "😀".repeat(PART_UNITS_MAX)));
    assert_eq!(found.len(), 2);
    assert_well_formed(&found);
    assert_eq!(found[0].plain.chars().count(), PART_UNITS_MAX / 2);
}

#[test]
fn pathological_inputs_never_panic_and_stay_well_formed() {
    let inputs = [
        "```".to_string(),
        "```rust\nnon chiuso **x".to_string(),
        "**non chiuso _ancora ~~e ancora".to_string(),
        "[link non chiuso](https://e.com".to_string(),
        "[a](<>)".to_string(),
        "<".repeat(10_000),
        "&".repeat(5_000),
        ">".repeat(20_000),
        "*".repeat(20_000),
        "_a".repeat(10_000),
        "`".repeat(9_999),
        "> ".repeat(5_000),
        "- ".repeat(3_000),
        "1. ".repeat(3_000),
        "    - [ ] \n".repeat(2_000),
        "| a |\n|---|\n".repeat(1_000),
        "# \n".repeat(3_000),
        "\u{0}\u{7}\u{200b}\u{feff}\r\n\r\n".repeat(1_000),
        "😀👩‍👩‍👧‍👦🇮🇹".repeat(3_000),
        format!("[x]({})", "a".repeat(50_000)),
        format!("{}{}", "[".repeat(1_000), "]".repeat(1_000)),
        "a\n".repeat(30_000),
        "18446744073709551615. massimo\n18446744073709551615. dopo".to_string(),
    ];
    for input in &inputs {
        let found = parts(input);
        assert_well_formed(&found);
    }
}
