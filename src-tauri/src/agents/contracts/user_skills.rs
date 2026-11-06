//! Contratto delle skill personali: dove l'agente legge le skill dell'utente.
//! Il core sincronizza le skill dell'utente in queste cartelle; un'integrazione
//! che non lo dichiara non riceve skill.

use std::path::PathBuf;

/// Cartella delle skill personali dell'agente.
pub trait UserSkills: Sync {
    /// Cartella delle skill personali su Windows.
    fn windows_dir(&self) -> Result<PathBuf, String>;

    /// Cartella delle skill relativa alla home Linux di una distro, slash dritti.
    fn wsl_home_subpath(&self) -> &'static str;

    /// Il nome con cui l'agente carica la skill personale `name` con il proprio
    /// strumento delle skill (e che il worker legge in `get_task`). Solo
    /// caratteri semplici (lettere, cifre e `-_.:`), senza prefissi di comando.
    fn skill_name(&self, name: &str) -> String;
}
