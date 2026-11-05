use super::*;

#[test]
fn utf8_is_the_default_and_invalid_bytes_are_replaced() {
    assert_eq!(decode("caffè".as_bytes(), "").as_deref(), Some("caffè"));
    assert_eq!(decode("caffè".as_bytes(), "UTF-8").as_deref(), Some("caffè"));
    assert_eq!(decode(&[b'a', 0xFF, b'b'], "utf-8").as_deref(), Some("a\u{FFFD}b"));
    assert_eq!(decode(b"", "utf-8").as_deref(), Some(""));
}

#[test]
fn latin1_maps_each_byte_to_a_char() {
    assert_eq!(decode(&[0x63, 0x61, 0x66, 0x66, 0xE8], "iso-8859-1").as_deref(), Some("caffè"));
    assert_eq!(decode(&[0xE8], "ISO-8859-1").as_deref(), Some("è"));
    assert_eq!(decode(&[0xE8], "latin1").as_deref(), Some("è"));
    assert_eq!(decode(b"abc", "us-ascii").as_deref(), Some("abc"));
}

#[test]
fn windows_1252_covers_the_high_range() {
    assert_eq!(decode(&[0x80, 0x93, 0x94, 0x96, 0xE8], "windows-1252").as_deref(), Some("€“”–è"));
    assert_eq!(decode(&[0x7F, 0x41], "windows-1252").as_deref(), Some("\u{7F}A"));
    assert_eq!(decode(&[0x81], "cp1252").as_deref(), Some("\u{81}"));
    assert_eq!(decode(&[0xFF], "windows-1252").as_deref(), Some("ÿ"));
}

#[test]
fn unknown_charsets_are_none_or_fall_back_to_utf8() {
    assert_eq!(decode(b"abc", "shift_jis"), None);
    assert_eq!(decode_or_utf8("è".as_bytes(), "shift_jis"), "è");
    assert_eq!(decode_or_utf8(b"abc", "iso-8859-1"), "abc");
}

#[test]
fn quoted_and_padded_charset_names_are_tolerated() {
    assert_eq!(decode(&[0xE8], " \"ISO-8859-1\" ").as_deref(), Some("è"));
}

#[test]
fn base64url_accepts_padding_whitespace_and_both_alphabets_of_the_url_variant() {
    assert_eq!(decode_base64url("Y2lhbw").unwrap(), b"ciao");
    assert_eq!(decode_base64url("Y2lhbw==").unwrap(), b"ciao");
    assert_eq!(decode_base64url("Y2l\nhbw ==\r\n").unwrap(), b"ciao");
    assert_eq!(decode_base64url("-_8").unwrap(), vec![0xFB, 0xFF]);
    assert_eq!(decode_base64url("").unwrap(), Vec::<u8>::new());
}

#[test]
fn base64url_rejects_garbage() {
    for bad in ["+/8", "a", "Y2l!", "caffè", "====a"] {
        assert!(decode_base64url(bad).is_none(), "{bad:?}");
    }
}
