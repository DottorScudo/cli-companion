//! Versione di OpenCode: `opencode --version` stampa `opencode v2.0.30`.

use super::SUPPORTED_MAJOR;

/// Primo token che, tolta una `v` iniziale, comincia con una cifra
/// (`opencode v2.0.30` -> `2.0.30`).
pub(super) fn parse_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| c == ',' || c == ';'))
        .map(|token| token.strip_prefix(['v', 'V']).unwrap_or(token))
        .find(|token| token.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
}

/// Versione maggiore (`2.0.30` -> 2).
fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

/// `true` se la versione rilevata ha l'API dei plugin che usiamo.
pub(super) fn is_supported(version: Option<&str>) -> bool {
    version.and_then(major) == Some(SUPPORTED_MAJOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_skips_the_program_name_and_the_v() {
        assert_eq!(parse_version("opencode v2.0.30\n").as_deref(), Some("2.0.30"));
        assert_eq!(parse_version("2.0.30").as_deref(), Some("2.0.30"));
        assert_eq!(parse_version("OpenCode V1.4.2, build").as_deref(), Some("1.4.2"));
        assert_eq!(parse_version("opencode"), None);
        assert_eq!(parse_version("vendor version"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn only_the_supported_major_is_supported() {
        assert!(is_supported(Some("2.0.30")));
        assert!(is_supported(Some("2.12.0")));
        for bad in [Some("1.14.3"), Some("3.0.0"), Some("20.0.0"), Some("x.y"), Some(""), None] {
            assert!(!is_supported(bad), "{bad:?}");
        }
    }
}
