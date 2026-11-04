//! Validatori condivisi: predicati sui valori che finiscono in comandi esterni
//! (branch, modello, repository) e l'errore di validazione tipizzato.
//! Funzioni pure, nessun I/O: le usano `board`, `automation` e `workspaces`
//! senza dipendere l'uno dall'altro.

mod error;
mod text;

pub use error::{ErrorCode, RuleList, ValidationError};
pub use text::{clip_chars, req_text};

/// Lunghezza massima del percorso di un repository (in caratteri).
pub const REPO_MAX: usize = 256;
/// Lunghezza massima del nome di un branch (in byte).
pub const BRANCH_MAX: usize = 100;
/// Lunghezza massima del nome di un modello (in byte).
pub const MODEL_MAX: usize = 64;
/// Caratteri del valore rifiutato citati in un messaggio.
const CLIPPED_VALUE_CHARS: usize = 40;

/// Nome di branch sicuro da passare come argv a git: niente `-` iniziale,
/// niente `..`, solo caratteri semplici.
pub fn valid_branch(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= BRANCH_MAX
        && !s.starts_with('-')
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.ends_with('.')
        && !s.contains("..")
        && !s.contains("//")
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

/// Nome di modello sicuro da passare come argomento all'agente.
pub fn valid_model(m: &str) -> bool {
    !m.is_empty()
        && m.len() <= MODEL_MAX
        && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Repository di un task: cartella relativa alla root. Normalizza i
/// separatori (`/`), toglie `./` iniziale e `/` finale; niente path assoluti
/// ne' `..`. `"."` (la root) vale `None`. Errori in inglese (strumenti MCP).
pub fn normalize_repo(s: &str) -> Result<Option<String>, String> {
    let t = s.trim().replace('\\', "/");
    if t.is_empty() {
        return Err("repo is empty (use \".\" for the workspace root, or null)".to_string());
    }
    if t.chars().count() > REPO_MAX || t.chars().any(char::is_control) {
        return Err(format!("invalid repo: {}", clip(&t)));
    }
    if is_absolute_like(&t) {
        return Err(format!("repo must be relative to the workspace root: {t}"));
    }
    let segs = path_segments(&t);
    if segs.contains(&"..") {
        return Err(format!("repo cannot contain '..': {t}"));
    }
    Ok(if segs.is_empty() { None } else { Some(segs.join("/")) })
}

/// Percorso assoluto in stile Unix (`/x`) o con lettera di unita' (`C:`).
pub fn is_absolute_like(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with('/') || (b.len() >= 2 && b[1] == b':')
}

/// Segmenti significativi di un percorso con `/`: niente vuoti ne' `.`.
pub fn path_segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty() && *s != ".").collect()
}

/// Primi caratteri di un valore, per citarlo in un messaggio.
pub fn clip(value: &str) -> String {
    value.chars().take(CLIPPED_VALUE_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches() {
        for ok in ["main", "release/1.0", "feat_x-2", "a.b"] {
            assert!(valid_branch(ok), "{ok}");
        }
        for bad in ["", "-x", "a..b", "a b", "x/", "/x", "a//b", "a.", &"a".repeat(BRANCH_MAX + 1), "è"] {
            assert!(!valid_branch(bad), "{bad}");
        }
        assert!(valid_branch(&"a".repeat(BRANCH_MAX)));
    }

    #[test]
    fn models() {
        assert!(valid_model("claude-opus-4.1_x") && valid_model(&"x".repeat(MODEL_MAX)));
        assert!(!valid_model("a b") && !valid_model(&"x".repeat(MODEL_MAX + 1)) && !valid_model(""));
    }

    #[test]
    fn repos_are_normalized() {
        assert_eq!(normalize_repo(" ./api/ ").unwrap(), Some("api".to_string()));
        assert_eq!(normalize_repo("a\\b").unwrap(), Some("a/b".to_string()));
        assert_eq!(normalize_repo(".").unwrap(), None);
        for bad in ["", "../x", "/abs", "C:\\x", "a/../b", "a\0b"] {
            assert!(normalize_repo(bad).is_err(), "{bad:?}");
        }
        assert!(normalize_repo(&"a".repeat(REPO_MAX + 1)).is_err());
        assert!(normalize_repo(&"a".repeat(REPO_MAX)).is_ok());
    }

    #[test]
    fn path_helpers() {
        assert!(is_absolute_like("/x") && is_absolute_like("C:/x") && !is_absolute_like("x/y"));
        assert_eq!(path_segments("./a//b/./c/"), ["a", "b", "c"]);
        assert_eq!(clip(&"é".repeat(60)).chars().count(), CLIPPED_VALUE_CHARS);
    }
}
