use super::*;

#[test]
fn the_challenge_matches_the_rfc_7636_appendix_b_example() {
    assert_eq!(
        challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn the_rfc_octets_give_the_rfc_verifier() {
    let octets = [
        116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37, 77, 105, 214, 191,
        240, 91, 88, 5, 88, 83, 132, 141, 121,
    ];
    let pkce = pkce_from_random(&octets).unwrap();
    assert_eq!(pkce.verifier, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
    assert_eq!(pkce.challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn the_verifier_length_is_checked() {
    assert!(pkce_from_random(&[]).is_none());
    assert!(pkce_from_random(&[7; 31]).is_none());
    assert_eq!(pkce_from_random(&[7; 32]).unwrap().verifier.len(), 43);
    assert_eq!(pkce_from_random(&[7; 96]).unwrap().verifier.len(), 128);
    assert!(pkce_from_random(&[7; 97]).is_none());
    assert!(pkce_from_random(&[7; 10_000]).is_none());
}

#[test]
fn verifier_and_challenge_use_the_url_safe_alphabet_without_padding() {
    let pkce = pkce_from_random(&[0xFB; PKCE_RANDOM_BYTES]).unwrap();
    for text in [&pkce.verifier, &pkce.challenge] {
        assert!(text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'), "{text}");
    }
    assert_eq!(pkce.challenge.len(), 43);
}

#[test]
fn debug_hides_the_verifier() {
    let pkce = pkce_from_random(&[1; 32]).unwrap();
    assert!(!format!("{pkce:?}").contains(&pkce.verifier));
}

#[test]
fn the_state_needs_enough_random_bytes() {
    assert!(state_from_random(&[1; 15]).is_none());
    assert!(state_from_random(&[]).is_none());
    assert_eq!(state_from_random(&[1; STATE_RANDOM_BYTES]).unwrap().len(), 43);
    assert!(state_from_random(&[1; 16]).is_some());
}

#[test]
fn states_are_compared_exactly() {
    assert!(states_match("abc", "abc"));
    assert!(states_match("", ""));
    assert!(!states_match("abc", "abd"));
    assert!(!states_match("abc", "abcd"));
    assert!(!states_match("abc", ""));
    assert!(!states_match("è", "e"));
}
