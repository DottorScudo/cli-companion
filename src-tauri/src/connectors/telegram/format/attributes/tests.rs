use super::*;

#[test]
fn only_known_schemes_with_a_target_are_links() {
    for target in ["https://example.com", "HTTP://EXAMPLE.COM/a", "mailto:nome@example.com", "tg://resolve?domain=bot"]
    {
        assert!(link_allowed(target), "{target}");
    }
    for target in [
        "",
        "example.com",
        "/percorso",
        "./README.md",
        "#ancora",
        "javascript:alert(1)",
        "JavaScript:alert(1)",
        "data:text/html,x",
        "file:///etc/passwd",
        "ftp://host",
        "https:",
        "https://",
        "https:///x",
        "https:example.com",
        "mailto:",
        "https://exa mple.com",
        "https://example.com/\n",
        "https://example.com/\u{7}",
    ] {
        assert!(!link_allowed(target), "{target:?}");
    }
}

#[test]
fn the_language_is_the_first_word_reduced_to_name_characters() {
    assert_eq!(code_language("rust"), Some("rust".to_string()));
    assert_eq!(code_language("  c++  extra parole"), Some("c++".to_string()));
    assert_eq!(code_language("objective-c"), Some("objective-c".to_string()));
    assert_eq!(code_language("\"><script>"), Some("script".to_string()));
    assert_eq!(code_language("rust,ignore"), Some("rustignore".to_string()));
    assert_eq!(code_language("{.class}"), Some("class".to_string()));
    assert_eq!(code_language(""), None);
    assert_eq!(code_language("   "), None);
    assert_eq!(code_language("{}"), None);
    assert_eq!(code_language("è"), None);
    assert_eq!(code_language(&"a".repeat(100)).map(|l| l.len()), Some(LANGUAGE_MAX));
}
