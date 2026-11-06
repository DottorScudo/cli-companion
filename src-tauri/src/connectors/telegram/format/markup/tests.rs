use super::*;

fn text(value: &str) -> String {
    let mut out = String::new();
    escape_text(value, &mut out);
    out
}

#[test]
fn text_escapes_only_what_telegram_requires() {
    assert_eq!(text("a & b < c > d \"e\" 'f'"), "a &amp; b &lt; c &gt; d \"e\" 'f'");
    assert_eq!(text("&amp;"), "&amp;amp;");
    assert_eq!(text("😀 è"), "😀 è");
}

#[test]
fn attributes_also_escape_quotes() {
    let link = Tag::Link { href: "https://e.com/?q=\"x\"&y=<z>".to_string() };
    assert_eq!(
        html([(std::slice::from_ref(&link), "l")]),
        "<a href=\"https://e.com/?q=&quot;x&quot;&amp;y=&lt;z&gt;\">l</a>"
    );
}

#[test]
fn every_tag_opens_and_closes() {
    let tags = [
        (Tag::Bold, "<b>x</b>"),
        (Tag::Italic, "<i>x</i>"),
        (Tag::Strike, "<s>x</s>"),
        (Tag::Code, "<code>x</code>"),
        (Tag::Pre { language: None }, "<pre>x</pre>"),
        (Tag::Pre { language: Some("rust".to_string()) }, "<pre><code class=\"language-rust\">x</code></pre>"),
        (Tag::Quote, "<blockquote>x</blockquote>"),
    ];
    for (tag, expected) in tags {
        assert_eq!(html([(std::slice::from_ref(&tag), "x")]), expected);
    }
}

#[test]
fn consecutive_pieces_share_their_common_tags() {
    let quote_bold = [Tag::Quote, Tag::Bold];
    let quote = [Tag::Quote];
    let pieces: [(&[Tag], &str); 4] = [(&quote_bold, "a"), (&quote_bold, "b"), (&quote, " c"), (&[], " d")];
    assert_eq!(html(pieces), "<blockquote><b>ab</b> c</blockquote> d");
}

#[test]
fn empty_pieces_open_no_tag() {
    let bold = [Tag::Bold];
    let pieces: [(&[Tag], &str); 2] = [(&bold, ""), (&[], "x")];
    assert_eq!(html(pieces), "x");
    assert_eq!(html(std::iter::empty()), "");
}
