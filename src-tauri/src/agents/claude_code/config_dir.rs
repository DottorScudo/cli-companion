//! Cartella di configurazione di Claude Code.

use std::path::PathBuf;

/// Cartella `.claude` su Windows: `CLAUDE_CONFIG_DIR` se impostata.
pub(crate) fn windows_claude_dir() -> Result<PathBuf, String> {
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty());
    if let Some(dir) = var("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    var("USERPROFILE")
        .or_else(|| var("HOME"))
        .map(|h| PathBuf::from(h).join(".claude"))
        .ok_or_else(|| "Cartella utente non trovata.".to_string())
}
