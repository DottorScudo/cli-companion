use super::*;

#[test]
fn plain_text_is_untouched() {
    for text in ["", "Ciao mondo", "caffè 😀", "a = b ? c", "=?", "?=", "=??=", "100%"] {
        assert_eq!(decode_words(text), text);
    }
}

#[test]
fn base64_words_are_decoded() {
    assert_eq!(decode_words("=?UTF-8?B?Y2FmZsOo?="), "caffè");
    assert_eq!(decode_words("=?utf-8?b?Y2FmZsOo?= <a@b.it>"), "caffè <a@b.it>");
    assert_eq!(decode_words("=?utf-8?B?Y2lhbw==?="), "ciao");
}

#[test]
fn quoted_printable_words_are_decoded() {
    assert_eq!(decode_words("=?iso-8859-1?Q?Caff=E8_freddo?="), "Caffè freddo");
    assert_eq!(decode_words("=?UTF-8?Q?Caff=C3=A8?="), "Caffè");
    assert_eq!(decode_words("=?windows-1252?Q?=93ciao=94?="), "“ciao”");
}

#[test]
fn text_around_words_is_kept() {
    assert_eq!(decode_words("Re: =?utf-8?B?Y2FmZsOo?= urgente"), "Re: caffè urgente");
}

#[test]
fn whitespace_between_adjacent_words_disappears() {
    assert_eq!(decode_words("=?utf-8?Q?ab?= =?utf-8?Q?cd?="), "abcd");
    assert_eq!(decode_words("=?utf-8?Q?ab?=\r\n =?utf-8?Q?cd?="), "abcd");
    assert_eq!(decode_words("=?utf-8?Q?ab?= x =?utf-8?Q?cd?="), "ab x cd");
}

#[test]
fn language_suffix_on_the_charset_is_ignored() {
    assert_eq!(decode_words("=?utf-8*it?Q?ciao?="), "ciao");
}

#[test]
fn unsupported_or_broken_words_are_left_as_they_are() {
    for text in [
        "=?shift_jis?B?gqA=?=",
        "=?utf-8?X?abc?=",
        "=?utf-8?Q?abc",
        "=?utf-8?B?!!!?=",
        "=?utf-8?Q?a=ZZb?=",
        "=?utf-8?Q?a=?=",
        "=?utf-8?B?",
        "=?utf-8?",
    ] {
        assert_eq!(decode_words(text), text, "{text}");
    }
}

#[test]
fn a_broken_word_does_not_hide_a_good_one_after_it() {
    assert_eq!(decode_words("=?x?X?a?= =?utf-8?Q?ok?="), "=?x?X?a?= ok");
}

#[test]
fn hostile_inputs_do_not_panic() {
    let long = "=?utf-8?Q?".repeat(10_000);
    let _ = decode_words(&long);
    let _ = decode_words(&"=?é?B?é?=".repeat(1000));
    let _ = decode_words("=?utf-8?Q?=é?=");
    let _ = decode_words("=?utf-8?Q?=\u{1F600}?=");
}
