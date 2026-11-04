//! Contratto `UserSkills` di OpenCode: le skill personali stanno in
//! `<config>/skills`: `XDG_CONFIG_HOME/opencode/skills` se impostata, altrimenti
//! `~/.config/opencode/skills` (anche su Windows e nella home Linux delle distro).
//! OpenCode legge anche la cartella delle skill di Claude Code: a parita' di id
//! vince la sua, quindi sincronizzare qui lo stesso contenuto non crea doppioni.

use super::OpenCode;
use crate::agents::UserSkills;
use std::ffi::OsString;
use std::path::PathBuf;

/// Cartella delle skill dentro la cartella `opencode` della configurazione.
const SKILLS_SUBPATH: &str = "opencode/skills";
/// Cartella di configurazione relativa alla home, quando `XDG_CONFIG_HOME` manca.
const DEFAULT_CONFIG_DIR: &str = ".config";

/// Cartella delle skill dalle variabili d'ambiente date (le vuote contano come assenti).
fn skills_dir(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Result<PathBuf, String> {
    let set = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
    let config = match set(xdg_config_home) {
        Some(xdg) => xdg,
        None => set(home).ok_or_else(|| "Cartella utente non trovata.".to_string())?.join(DEFAULT_CONFIG_DIR),
    };
    Ok(config.join(SKILLS_SUBPATH))
}

impl UserSkills for OpenCode {
    fn windows_dir(&self) -> Result<PathBuf, String> {
        let var = |name: &str| std::env::var_os(name);
        skills_dir(var("XDG_CONFIG_HOME"), var("USERPROFILE").or_else(|| var("HOME")))
    }

    fn wsl_home_subpath(&self) -> &'static str {
        ".config/opencode/skills"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn xdg_config_home_wins_over_the_home() {
        assert_eq!(skills_dir(os("/x/cfg"), os("/home/me")), Ok(PathBuf::from("/x/cfg").join("opencode/skills")));
    }

    #[test]
    fn without_xdg_the_skills_live_under_dot_config_of_the_home() {
        let expected = PathBuf::from("/home/me").join(".config").join("opencode/skills");
        assert_eq!(skills_dir(None, os("/home/me")), Ok(expected.clone()));
        assert_eq!(skills_dir(os(""), os("/home/me")), Ok(expected));
    }

    #[test]
    fn no_home_and_no_xdg_is_an_error() {
        assert!(skills_dir(None, None).is_err());
        assert!(skills_dir(os(""), os("")).is_err());
    }

    #[test]
    fn distro_skills_live_in_the_dot_config_folder_of_the_home() {
        assert_eq!(OpenCode.wsl_home_subpath(), ".config/opencode/skills");
    }
}
