use super::*;

fn failure(status: u16, retry_after: Option<&str>, body: &[u8]) -> ApiFailure {
    classify(status, retry_after, body).unwrap_err()
}

fn error(status: u16, retry_after: Option<&str>, body: &[u8]) -> ConnectorError {
    match failure(status, retry_after, body) {
        ApiFailure::Error(e) => e,
        other => panic!("atteso Error, trovato {other:?}"),
    }
}

#[test]
fn success_statuses_are_ok() {
    for status in [200, 201, 204, 299] {
        assert_eq!(classify(status, None, b""), Ok(()));
    }
}

#[test]
fn simple_statuses_have_their_own_failures() {
    assert_eq!(failure(401, None, b""), ApiFailure::Unauthorized);
    assert_eq!(failure(404, None, b"{}"), ApiFailure::NotFound);
    assert_eq!(failure(400, None, b"garbage"), ApiFailure::BadRequest);
}

#[test]
fn too_many_requests_honours_retry_after_seconds() {
    assert_eq!(error(429, Some("30"), b""), ConnectorError::RateLimited { retry_after: Some(Duration::from_secs(30)) });
    assert_eq!(error(429, Some(" 7 "), b""), ConnectorError::RateLimited { retry_after: Some(Duration::from_secs(7)) });
    for raw in [
        None,
        Some(""),
        Some("soon"),
        Some("-5"),
        Some("1.5"),
        Some("Wed, 21 Oct 2015 07:28:00 GMT"),
        Some("99999999999999999999"),
    ] {
        assert_eq!(error(429, raw, b""), ConnectorError::RateLimited { retry_after: None }, "{raw:?}");
    }
}

#[test]
fn a_403_for_rate_limits_is_rate_limited() {
    let by_errors = br#"{"error":{"code":403,"errors":[{"domain":"usageLimits","reason":"userRateLimitExceeded"}]}}"#;
    let by_status = br#"{"error":{"code":403,"status":"RATE_LIMIT_EXCEEDED"}}"#;
    let by_details = br#"{"error":{"code":403,"details":[{"@type":"x","reason":"RATE_LIMIT_EXCEEDED"}]}}"#;
    let plain = br#"{"error":{"errors":[{"reason":"rateLimitExceeded"}]}}"#;
    for body in [&by_errors[..], by_status, by_details, plain] {
        assert!(matches!(error(403, Some("5"), body), ConnectorError::RateLimited { retry_after: Some(_) }));
    }
}

#[test]
fn a_403_for_a_disabled_api_is_a_config_error() {
    let not_configured = br#"{"error":{"errors":[{"reason":"accessNotConfigured"}]}}"#;
    let disabled = br#"{"error":{"status":"PERMISSION_DENIED","details":[{"reason":"SERVICE_DISABLED"}]}}"#;
    for body in [&not_configured[..], disabled] {
        assert_eq!(
            error(403, None, body),
            ConnectorError::Config("Gmail API non abilitata nel progetto Google Cloud".to_string())
        );
    }
}

#[test]
fn any_other_403_is_an_auth_error() {
    let insufficient = br#"{"error":{"errors":[{"reason":"insufficientPermissions"}],"status":"PERMISSION_DENIED"}}"#;
    for body in [&insufficient[..], b"", b"<html>", b"{}", br#"{"error":"x"}"#, br#"{"error":{"errors":"x"}}"#, b"[]"] {
        assert!(matches!(error(403, None, body), ConnectorError::Auth(_)), "{body:?}");
    }
}

#[test]
fn server_errors_and_odd_statuses_are_transient() {
    for status in [500, 502, 503, 504, 302, 100, 418, 409, 0, 999] {
        assert!(matches!(error(status, None, b""), ConnectorError::Transient(_)), "{status}");
    }
}

#[test]
fn messages_never_echo_the_body() {
    let body = br#"{"error":{"message":"leaked-secret-123","errors":[{"reason":"other"}]}}"#;
    for status in [403, 500] {
        assert!(!error(status, None, body).to_string().contains("leaked-secret-123"));
    }
}
