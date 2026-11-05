use super::*;

#[test]
fn plain_text_and_empty_input() {
    assert_eq!(html_to_text(""), "");
    assert_eq!(html_to_text("   \n\t "), "");
    assert_eq!(html_to_text("solo testo"), "solo testo");
    assert_eq!(html_to_text("caffè 😀"), "caffè 😀");
}

#[test]
fn tags_are_stripped_and_blocks_break_lines() {
    assert_eq!(html_to_text("<p>uno</p><p>due</p>"), "uno\n\ndue");
    assert_eq!(html_to_text("a<br>b<br/>c<BR />d"), "a\nb\nc\nd");
    assert_eq!(html_to_text("<div>x</div><div>y</div>"), "x\n\ny");
    assert_eq!(html_to_text("<b>grassetto</b> e <i>corsivo</i>"), "grassetto e corsivo");
    assert_eq!(html_to_text("<h1>Titolo</h1>testo"), "Titolo\ntesto");
}

#[test]
fn lists_get_dashes() {
    assert_eq!(html_to_text("<ul><li>uno</li><li>due</li></ul>"), "- uno\n- due");
}

#[test]
fn table_cells_are_separated() {
    assert_eq!(html_to_text("<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>"), "a b\n\nc d");
}

#[test]
fn script_style_head_and_comments_are_dropped() {
    let html = "<html><head><title>T</title><style>p{}</style></head><body>ciao<script>var x = '<b>';</script> mondo<!-- nascosto --></body></html>";
    assert_eq!(html_to_text(html), "ciao mondo");
    assert_eq!(html_to_text("a<SCRIPT type=\"x\">alert(1)</SCRIPT>b"), "ab");
    assert_eq!(html_to_text("a<!-- <p>x</p> -->b"), "ab");
    assert_eq!(html_to_text("<!DOCTYPE html>ciao"), "ciao");
}

#[test]
fn unterminated_constructs_do_not_leak_or_panic() {
    assert_eq!(html_to_text("ciao<script>codice senza fine"), "ciao");
    assert_eq!(html_to_text("ciao<!-- senza fine"), "ciao");
    assert_eq!(html_to_text("ciao<b attr=\"x"), "ciao<b attr=\"x");
    assert_eq!(html_to_text("ciao <"), "ciao <");
    assert_eq!(html_to_text("a < b e c > d"), "a < b e c > d");
    assert_eq!(html_to_text("<head>solo"), "");
}

#[test]
fn quoted_attributes_may_contain_a_greater_than_sign() {
    assert_eq!(html_to_text("<a href=\"x>y\" title='a>b'>link</a>"), "link");
}

#[test]
fn named_entities_are_decoded() {
    assert_eq!(html_to_text("a &amp; b &lt;c&gt; &quot;d&quot; &#39;e&apos; f&nbsp;g"), "a & b <c> \"d\" 'e' f g");
}

#[test]
fn numeric_entities_are_decoded() {
    assert_eq!(html_to_text("&#65;&#x42;&#X43; &#x1F600; &#233;"), "ABC 😀 é");
}

#[test]
fn invalid_entities_stay_literal() {
    for text in ["&", "&;", "&foo;", "&#;", "&#xZZ;", "&#99999999999;", "&#xD800;", "&#0;", "&amp", "AT&T", "&#1;"] {
        assert_eq!(html_to_text(text), text, "{text}");
    }
}

#[test]
fn an_escaped_tag_is_text_not_markup() {
    assert_eq!(html_to_text("&lt;script&gt;x&lt;/script&gt;"), "<script>x</script>");
}

#[test]
fn whitespace_is_collapsed_and_blank_lines_limited() {
    assert_eq!(html_to_text("a    b\t\tc"), "a b c");
    assert_eq!(html_to_text("a\n\n\n\n\nb"), "a\n\nb");
    assert_eq!(html_to_text("<p>a</p><p></p><p></p><p>b</p>"), "a\n\nb");
    assert_eq!(html_to_text("  <p>  a  </p>  "), "a");
}

#[test]
fn a_huge_input_is_bounded_and_fast() {
    let html = "<p>riga</p>".repeat(200_000);
    let text = html_to_text(&html);
    assert!(text.chars().count() < HTML_CHARS_MAX);
    let deep = "<div>".repeat(300_000);
    let _ = html_to_text(&deep);
    let lt = "<".repeat(300_000);
    assert_eq!(html_to_text(&lt).len(), 300_000);
    let quotes = "<a\"".repeat(200_000);
    assert_eq!(html_to_text(&quotes).chars().count(), HTML_CHARS_MAX);
}

#[test]
fn unicode_boundaries_never_panic() {
    let _ = html_to_text("<é>è</é>&é;&#é;<😀 a='😀'>😀</😀>");
    let _ = html_to_text(&"😀<".repeat(1000));
}
