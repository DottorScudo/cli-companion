use super::*;

#[test]
fn the_keys_are_the_documented_ones() {
    assert_eq!(ID, "gmail");
    assert_eq!(CLIENT_ID_KEY, "client_id");
    assert_eq!(INTERVAL_KEY, "interval_minutes");
    assert_eq!(CLIENT_SECRET_SLOT, "client_secret");
    assert_eq!(REFRESH_TOKEN_SLOT, "refresh_token");
    assert_eq!(QUERY_KEY, "query");
}

#[test]
fn hosts_are_bare_names_and_the_scope_is_read_only() {
    for host in [ACCOUNTS_HOST, OAUTH_HOST, API_HOST] {
        assert!(!host.contains('/') && !host.contains(':'), "{host}");
    }
    assert!(SCOPE.ends_with("/gmail.readonly"));
}
