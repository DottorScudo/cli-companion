use super::*;

#[test]
fn the_redirect_is_the_loopback_with_the_port() {
    assert_eq!(redirect_uri(8123), "http://127.0.0.1:8123");
    assert_eq!(redirect_uri(0), "http://127.0.0.1:0");
}

#[test]
fn the_consent_url_carries_every_parameter_encoded() {
    let url = consent_url("id.apps.googleusercontent.com", "http://127.0.0.1:5000", "st&te", "chal-lenge_1");
    assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    for part in [
        "client_id=id.apps.googleusercontent.com",
        "redirect_uri=http%3A%2F%2F127.0.0.1%3A5000",
        "response_type=code",
        "scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.readonly",
        "code_challenge=chal-lenge_1",
        "code_challenge_method=S256",
        "state=st%26te",
        "access_type=offline",
        "prompt=consent",
    ] {
        assert!(url.contains(part), "manca {part} in {url}");
    }
    assert_eq!(url.matches('?').count(), 1);
}

#[test]
fn hostile_values_cannot_inject_parameters() {
    let url = consent_url("a&scope=evil", "x#y", "s s", "c=d");
    assert!(!url.contains("scope=evil"));
    assert!(!url.contains('#'));
    assert!(!url.contains(' '));
    assert_eq!(url.matches("scope=").count(), 1);
}
