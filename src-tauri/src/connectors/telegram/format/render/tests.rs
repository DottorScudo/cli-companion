use super::*;
use crate::connectors::telegram::format::markup::html;

fn convert(markdown: &str) -> String {
    html(runs(markdown).iter().map(|run| (run.tags.as_slice(), run.text.as_str())))
}

fn visible(markdown: &str) -> String {
    runs(markdown).iter().map(|run| run.text.as_str()).collect()
}

#[test]
fn special_characters_are_escaped_everywhere() {
    assert_eq!(convert("a & b < c > d"), "a &amp; b &lt; c &gt; d");
    assert_eq!(convert("**a & <b>**"), "<b>a &amp; &lt;b&gt;</b>");
    assert_eq!(convert("`x < y && z`"), "<code>x &lt; y &amp;&amp; z</code>");
    assert_eq!(convert("\"citato\""), "\"citato\"");
}

#[test]
fn raw_html_never_passes_and_shows_as_text() {
    assert_eq!(
        convert("<b>finto</b> e <script>alert(1)</script>"),
        "&lt;b&gt;finto&lt;/b&gt; e &lt;script&gt;alert(1)&lt;/script&gt;"
    );
    assert_eq!(convert("<div>\nblocco\n</div>"), "&lt;div&gt;\nblocco\n&lt;/div&gt;");
    assert_eq!(convert("&lt;gia' escapato&gt; &amp;"), "&lt;gia' escapato&gt; &amp;");
}

#[test]
fn bold_italic_strike_and_inline_code() {
    assert_eq!(convert("**grassetto** e __anche__"), "<b>grassetto</b> e <b>anche</b>");
    assert_eq!(convert("*corsivo* e _anche_"), "<i>corsivo</i> e <i>anche</i>");
    assert_eq!(convert("~~barrato~~"), "<s>barrato</s>");
    assert_eq!(convert("usa `cargo test`"), "usa <code>cargo test</code>");
    assert_eq!(convert("***tutti e due***"), "<i><b>tutti e due</b></i>");
}

#[test]
fn intraword_underscores_and_stars_stay_text() {
    assert_eq!(convert("snake_case_name e a*b*c no"), "snake_case_name e a<i>b</i>c no");
    assert_eq!(convert("snake_case_name"), "snake_case_name");
    assert_eq!(convert("2 * 3 * 4"), "2 * 3 * 4");
    assert_eq!(convert("file_a_b.rs"), "file_a_b.rs");
}

#[test]
fn a_nested_same_tag_is_not_opened_twice() {
    assert_eq!(convert("# Titolo **forte**"), "<b>Titolo forte</b>");
    assert_eq!(convert("**a `b` c**"), "<b>a <code>b</code> c</b>");
}

#[test]
fn fenced_code_keeps_its_content_literal_with_the_language() {
    assert_eq!(
        convert("```rust\nfn main() { let a = **b** < 1; }\n```"),
        "<pre><code class=\"language-rust\">fn main() { let a = **b** &lt; 1; }</code></pre>"
    );
    assert_eq!(convert("```\nsenza linguaggio\n\nseconda\n```"), "<pre>senza linguaggio\n\nseconda</pre>");
    assert_eq!(convert("    indentato\n    *non corsivo*"), "<pre>indentato\n*non corsivo*</pre>");
}

#[test]
fn the_code_language_is_cleaned_or_dropped() {
    assert_eq!(convert("```c++ extra\nx\n```"), "<pre><code class=\"language-c++\">x</code></pre>");
    assert_eq!(convert("```\"><script>\nx\n```"), "<pre><code class=\"language-script\">x</code></pre>");
    assert_eq!(convert("```{}\nx\n```"), "<pre>x</pre>");
}

#[test]
fn an_empty_code_block_writes_nothing() {
    assert_eq!(convert("prima\n\n```\n```\n\ndopo"), "prima\n\ndopo");
}

#[test]
fn headings_become_bold_lines() {
    assert_eq!(convert("# Titolo\n\nTesto"), "<b>Titolo</b>\n\nTesto");
    assert_eq!(convert("###### Piccolo\nsubito sotto"), "<b>Piccolo</b>\nsubito sotto");
    assert_eq!(convert("Setext\n======"), "<b>Setext</b>");
}

#[test]
fn bullet_lists_use_bullets_and_nested_levels_are_indented() {
    assert_eq!(
        convert("- uno\n- due\n  - due.a\n    - profondo\n- tre"),
        "• uno\n• due\n  • due.a\n    • profondo\n• tre"
    );
    assert_eq!(convert("* **forte** voce"), "• <b>forte</b> voce");
}

#[test]
fn ordered_lists_keep_their_numbers_from_the_start() {
    assert_eq!(convert("3. tre\n4. quattro\n1. cinque"), "3. tre\n4. quattro\n5. cinque");
    assert_eq!(convert("1. a\n   - b\n2. c"), "1. a\n  • b\n2. c");
}

#[test]
fn task_lists_use_check_boxes() {
    assert_eq!(convert("- [ ] da fare\n- [x] fatto"), "☐ da fare\n☑ fatto");
    assert_eq!(convert("- [ ] da fare\n\n- [x] fatto"), "☐ da fare\n\n☑ fatto");
    assert_eq!(convert("1. [x] primo"), "1. ☑ primo");
}

#[test]
fn a_list_after_a_paragraph_keeps_the_original_spacing() {
    assert_eq!(convert("Elenco:\n- a\n- b"), "Elenco:\n• a\n• b");
    assert_eq!(convert("Elenco:\n\n- a\n\nFine."), "Elenco:\n\n• a\n\nFine.");
}

#[test]
fn quotes_become_blockquotes_without_nesting() {
    assert_eq!(convert("> citato\n> ancora"), "<blockquote>citato\nancora</blockquote>");
    assert_eq!(convert("> uno\n>\n> due"), "<blockquote>uno\n\ndue</blockquote>");
    assert_eq!(convert("> fuori\n>> dentro"), "<blockquote>fuori\ndentro</blockquote>");
    assert_eq!(convert("prima\n\n> **forte**\n\ndopo"), "prima\n\n<blockquote><b>forte</b></blockquote>\n\ndopo");
}

#[test]
fn allowed_links_are_anchors() {
    assert_eq!(
        convert("[sito](https://example.com/a?b=1&c=\"2\")"),
        "<a href=\"https://example.com/a?b=1&amp;c=&quot;2&quot;\">sito</a>"
    );
    assert_eq!(convert("<https://example.com>"), "<a href=\"https://example.com\">https://example.com</a>");
    assert_eq!(convert("<nome@example.com>"), "<a href=\"mailto:nome@example.com\">nome@example.com</a>");
    assert_eq!(convert("[canale](tg://resolve?domain=x)"), "<a href=\"tg://resolve?domain=x\">canale</a>");
    assert_eq!(convert("[](https://example.com)"), "<a href=\"https://example.com\">https://example.com</a>");
}

#[test]
fn links_carry_no_formatting_inside() {
    assert_eq!(convert("[**forte** `x`](https://e.com)"), "<a href=\"https://e.com\">forte x</a>");
}

#[test]
fn other_schemes_and_relative_links_stay_text() {
    assert_eq!(convert("[clic](javascript:alert(1))"), "clic (javascript:alert(1))");
    assert_eq!(convert("[docs](./README.md)"), "docs (./README.md)");
    assert_eq!(convert("[/x](/x)"), "/x");
    assert_eq!(convert("<ftp://host/file>"), "ftp://host/file");
}

#[test]
fn images_are_links_with_their_alt_text() {
    assert_eq!(convert("![schema](https://e.com/a.png)"), "<a href=\"https://e.com/a.png\">schema</a>");
    assert_eq!(convert("![](https://e.com/a.png)"), "<a href=\"https://e.com/a.png\">https://e.com/a.png</a>");
    assert_eq!(convert("![logo](logo.png)"), "logo (logo.png)");
    assert_eq!(convert("[![alt](https://e.com/i.png)](https://e.com)"), "<a href=\"https://e.com\">alt</a>");
}

#[test]
fn tables_become_a_readable_pre_block() {
    assert_eq!(
        convert("| Nome | Stato |\n|---|:-:|\n| **a** | ok `x` |\n| b | [l](https://e.com) |\n\nDopo"),
        "<pre>Nome | Stato\na | ok x\nb | l</pre>\n\nDopo"
    );
}

#[test]
fn a_rule_is_a_line_of_dashes() {
    assert_eq!(convert("sopra\n\n---\n\nsotto"), "sopra\n\n———\n\nsotto");
    assert_eq!(convert("***"), "———");
}

#[test]
fn soft_and_hard_breaks_stay_line_breaks() {
    assert_eq!(convert("riga uno\nriga due\nriga tre"), "riga uno\nriga due\nriga tre");
    assert_eq!(convert("con spazi  \ndopo"), "con spazi\ndopo");
    assert_eq!(convert("primo\n\n\n\nsecondo"), "primo\n\nsecondo");
}

#[test]
fn plain_app_texts_are_unchanged() {
    for text in [
        "Comandi:\n/threads: i thread aperti\n/use <n>: scegli il thread dall'ultimo elenco\n\
         /ws [n|nome]: scegli il workspace dove /new apre i thread\n/new <titolo>: apri un thread nuovo\n\n\
         Un messaggio normale va nel thread corrente.",
        "1. ⏳ Lavoro › Piano\n2. Generale › Idee_nuove\n\nScegli con /use <n>.",
        "1. ✅ lavoro\n2. casa\n\nScegli con /ws <n> o /ws <nome>: /new apre i thread li'.",
        "Uso: /use <n>, con n da 1 a 3 (dall'ultimo elenco di /threads).",
        "Thread corrente: Lavoro › Piano\nThread che aspettano te: 2 (Lavoro: 1, Casa: 1).\nNotifiche attive.",
        "Uso: /mute [minuti], con minuti da 1 a 1440. Senza numero silenzio finché non scrivi /unmute.",
        "Scrivi nella conversazione «Lavoro › Piano»: i messaggi lì vanno in quel thread.",
        "Allegati non importati: foto.jpg (troppo grande). Limiti: 20 MB per file, 10 per messaggio.",
        "Testo lungo (parte 2/3)\n\nseguito",
    ] {
        assert_eq!(visible(text), text, "{text}");
        assert!(runs(text).iter().all(|run| run.tags.is_empty()), "{text}");
    }
}
