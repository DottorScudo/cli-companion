use super::*;

#[test]
fn unreserved_characters_are_kept_and_the_rest_is_encoded() {
    assert_eq!(encode_component("aZ09-._~"), "aZ09-._~");
    assert_eq!(encode_component("a b&c=d/e?f+g"), "a%20b%26c%3Dd%2Fe%3Ff%2Bg");
    assert_eq!(encode_component("caffè 😀"), "caff%C3%A8%20%F0%9F%98%80");
    assert_eq!(encode_component(""), "");
}

#[test]
fn the_body_joins_encoded_pairs() {
    let body = form_body(&[("grant_type", "refresh_token"), ("x y", "a&b")]);
    assert_eq!(String::from_utf8(body).unwrap(), "grant_type=refresh_token&x%20y=a%26b");
    assert!(form_body(&[]).is_empty());
}

#[test]
fn decoding_handles_plus_percent_and_unicode() {
    assert_eq!(decode_component("a+b%20c").as_deref(), Some("a b c"));
    assert_eq!(decode_component("caff%C3%A8").as_deref(), Some("caffè"));
    assert_eq!(decode_component("").as_deref(), Some(""));
    assert_eq!(decode_component("%2B").as_deref(), Some("+"));
}

#[test]
fn decoding_rejects_invalid_utf8_and_keeps_broken_escapes_literal() {
    assert_eq!(decode_component("%FF%FE"), None);
    assert_eq!(decode_component("100%").as_deref(), Some("100%"));
    assert_eq!(decode_component("%zz").as_deref(), Some("%zz"));
}

#[test]
fn encode_then_decode_round_trips() {
    for text in ["", "a b+c", "è😀/&=?%", "~._-"] {
        assert_eq!(decode_component(&encode_component(text)).as_deref(), Some(text));
    }
}
