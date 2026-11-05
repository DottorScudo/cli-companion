use super::*;

fn grant(token: &str, expires_in_secs: u64) -> TokenGrant {
    TokenGrant { access_token: token.to_string(), expires_in_secs, refresh_token: None, scope: None }
}

#[test]
fn a_token_is_valid_until_the_margin_before_expiry() {
    let mut cache = TokenCache::new();
    cache.put("i1", &grant("tok", 3600), 1_000);
    let expires = 1_000 + 3_600_000;
    assert_eq!(cache.get("i1", 1_000).as_deref(), Some("tok"));
    assert_eq!(cache.get("i1", expires - EXPIRY_MARGIN_MS - 1).as_deref(), Some("tok"));
    assert_eq!(cache.get("i1", expires - EXPIRY_MARGIN_MS), None);
    assert_eq!(cache.get("i1", expires + 5), None);
}

#[test]
fn unknown_instances_and_short_tokens_miss() {
    let mut cache = TokenCache::new();
    assert_eq!(cache.get("nope", 0), None);
    cache.put("i1", &grant("tok", 30), 0);
    assert_eq!(cache.get("i1", 0), None);
    cache.put("i2", &grant("tok", 0), 0);
    assert_eq!(cache.get("i2", 0), None);
}

#[test]
fn put_replaces_and_invalidate_forgets() {
    let mut cache = TokenCache::new();
    cache.put("i1", &grant("old", 3600), 0);
    cache.put("i1", &grant("new", 3600), 0);
    assert_eq!(cache.get("i1", 0).as_deref(), Some("new"));
    cache.invalidate("i1");
    assert_eq!(cache.get("i1", 0), None);
    cache.invalidate("i1");
}

#[test]
fn instances_do_not_share_tokens() {
    let mut cache = TokenCache::new();
    cache.put("a", &grant("ta", 3600), 0);
    cache.put("b", &grant("tb", 3600), 0);
    assert_eq!(cache.get("a", 0).as_deref(), Some("ta"));
    assert_eq!(cache.get("b", 0).as_deref(), Some("tb"));
}

#[test]
fn the_map_is_bounded_and_the_soonest_expiring_goes_first() {
    let mut cache = TokenCache::new();
    for n in 0..MAX_INSTANCES {
        cache.put(&format!("i{n}"), &grant("t", 3600 + n as u64), 0);
    }
    cache.put("extra", &grant("t", 7200), 0);
    assert_eq!(cache.entries.len(), MAX_INSTANCES);
    assert_eq!(cache.get("i0", 0), None);
    assert!(cache.get("i1", 0).is_some());
    assert!(cache.get("extra", 0).is_some());
}

#[test]
fn the_new_entry_survives_even_when_it_expires_first() {
    let mut cache = TokenCache::new();
    for n in 0..MAX_INSTANCES {
        cache.put(&format!("i{n}"), &grant("t", 7200), 0);
    }
    cache.put("short", &grant("t", 1000), 0);
    assert_eq!(cache.entries.len(), MAX_INSTANCES);
    assert!(cache.get("short", 0).is_some());
}

#[test]
fn extreme_values_do_not_overflow() {
    let mut cache = TokenCache::new();
    cache.put("i", &grant("t", u64::MAX), i64::MAX - 5);
    assert_eq!(cache.get("i", i64::MAX - 1_000_000).as_deref(), Some("t"));
    assert_eq!(cache.get("i", i64::MIN).as_deref(), Some("t"));
}

#[test]
fn debug_hides_the_token() {
    let mut cache = TokenCache::new();
    cache.put("i", &grant("ya29.SECRET", 3600), 0);
    assert!(!format!("{cache:?}").contains("SECRET"));
}

#[test]
fn it_can_live_in_a_static_mutex() {
    static CACHE: std::sync::Mutex<TokenCache> = std::sync::Mutex::new(TokenCache::new());
    CACHE.lock().unwrap().put("i", &grant("t", 3600), 0);
    assert!(CACHE.lock().unwrap().get("i", 0).is_some());
}
