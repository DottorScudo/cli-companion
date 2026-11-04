//! Contratto delle skill personali: dove l'agente legge le skill dell'utente.

use std::path::PathBuf;

/// Cartella delle skill personali dell'agente.
pub trait UserSkills: Sync {
    /// Cartella delle skill personali su Windows.
    #[expect(dead_code, reason = "wired by a later task (agent-boundaries)")]
    fn windows_dir(&self) -> Result<PathBuf, String>;

    /// Cartella delle skill relativa alla home Linux di una distro, slash dritti.
    #[cfg_attr(not(test), expect(dead_code, reason = "wired by a later task (agent-boundaries)"))]
    fn wsl_home_subpath(&self) -> &'static str;
}
