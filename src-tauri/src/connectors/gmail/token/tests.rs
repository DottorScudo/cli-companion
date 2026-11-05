use super::*;

fn text(body: Vec<u8>) -> String {
    String::from_utf8(body).unwrap()
}

#[test]
fn the_exchange_body_carries_the_code_and_the_verifier() {
    let body = text(code_exchange_body("4/0A b", "cid", "sec&ret", "http://127.0.0.1:5000", "ver"));
    assert_eq!(
        body,
        "grant_type=authorization_code&code=4%2F0A%20b&client_id=cid&client_secret=sec%26ret\
         &redirect_uri=http%3A%2F%2F127.0.0.1%3A5000&code_verifier=ver"
    );
}

#[test]
fn the_refresh_body_uses_the_refresh_grant() {
    let body = text(refresh_body("1//rt", "cid", "sec"));
    assert_eq!(body, "grant_type=refresh_token&refresh_token=1%2F%2Frt&client_id=cid&client_secret=sec");
}

#[test]
fn a_full_grant_is_parsed() {
    let body = br#"{"access_token":"ya29.abc","expires_in":3599,"refresh_token":"1//rt","scope":"https://www.googleapis.com/auth/gmail.readonly","token_type":"Bearer"}"#;
    let grant = parse_token_response(200, body).unwrap();
    assert_eq!(grant.access_token, "ya29.abc");
    assert_eq!(grant.expires_in_secs, 3599);
    assert_eq!(grant.refresh_token.as_deref(), Some("1//rt"));
    assert!(grants_scope(&grant));
}

#[test]
fn expiry_defaults_and_is_clamped() {
    let none = parse_token_response(200, br#"{"access_token":"t"}"#).unwrap();
    assert_eq!(none.expires_in_secs, 3600);
    assert!(none.refresh_token.is_none());
    let huge = parse_token_response(200, br#"{"access_token":"t","expires_in":99999999999}"#).unwrap();
    assert_eq!(huge.expires_in_secs, 86_400);
    let negative = parse_token_response(200, br#"{"access_token":"t","expires_in":-5}"#).unwrap();
    assert_eq!(negative.expires_in_secs, 3600);
}

#[test]
fn invalid_successful_bodies_are_transient() {
    let huge = format!(r#"{{"access_token":"{}"}}"#, "a".repeat(5000));
    let bodies: Vec<&[u8]> = vec![
        b"",
        b"not json",
        b"[]",
        b"{}",
        br#"{"access_token":""}"#,
        br#"{"access_token":5}"#,
        br#"{"access_token":"a b"}"#,
        "{\"access_token\":\"caffè\"}".as_bytes(),
        br#"{"access_token":"t","refresh_token":"bad token"}"#,
        huge.as_bytes(),
    ];
    for body in bodies {
        assert!(matches!(parse_token_response(200, body), Err(ConnectorError::Transient(_))), "{body:?}");
    }
}

#[test]
fn a_null_refresh_token_is_absent() {
    let grant = parse_token_response(200, br#"{"access_token":"t","refresh_token":null}"#).unwrap();
    assert!(grant.refresh_token.is_none());
}

#[test]
fn oauth_errors_are_classified() {
    let err = |status, body: &[u8]| parse_token_response(status, body).unwrap_err();
    assert!(matches!(
        err(400, br#"{"error":"invalid_grant","error_description":"Bad Request"}"#),
        ConnectorError::Auth(_)
    ));
    assert!(matches!(err(401, br#"{"error":"invalid_client"}"#), ConnectorError::Auth(_)));
    assert!(matches!(err(401, br#"{"error":"unauthorized_client"}"#), ConnectorError::Auth(_)));
    assert!(matches!(err(400, br#"{"error":"invalid_request"}"#), ConnectorError::Fatal(_)));
    assert!(matches!(err(400, b"garbage"), ConnectorError::Fatal(_)));
    assert!(matches!(err(403, b""), ConnectorError::Fatal(_)));
    assert_eq!(err(429, b""), ConnectorError::RateLimited { retry_after: None });
    for status in [500, 502, 503, 302, 100] {
        assert!(matches!(err(status, br#"{"error":"invalid_grant"}"#), ConnectorError::Transient(_)), "{status}");
    }
}

#[test]
fn error_messages_never_echo_the_body() {
    let body = br#"{"error":"invalid_grant","error_description":"secret-detail-xyz"}"#;
    let message = parse_token_response(400, body).unwrap_err().to_string();
    assert!(!message.contains("secret-detail-xyz"));
}

#[test]
fn debug_hides_every_token() {
    let grant = parse_token_response(200, br#"{"access_token":"ya29.SECRET","refresh_token":"1//SECRETRT"}"#).unwrap();
    let shown = format!("{grant:?}");
    assert!(!shown.contains("SECRET"));
    assert!(shown.contains("expires_in_secs"));
}

#[test]
fn the_scope_check_looks_for_the_exact_scope() {
    let with = |scope: Option<&str>| TokenGrant {
        access_token: "t".into(),
        expires_in_secs: 1,
        refresh_token: None,
        scope: scope.map(str::to_string),
    };
    assert!(grants_scope(&with(None)));
    assert!(grants_scope(&with(Some("openid https://www.googleapis.com/auth/gmail.readonly email"))));
    assert!(!grants_scope(&with(Some("openid email"))));
    assert!(!grants_scope(&with(Some(""))));
    assert!(!grants_scope(&with(Some("https://www.googleapis.com/auth/gmail.readonly.evil"))));
}

#[test]
fn the_token_endpoint_is_on_the_declared_host() {
    assert!(TOKEN_URL.starts_with(&format!("https://{}/", crate::connectors::gmail::keys::OAUTH_HOST)));
}
