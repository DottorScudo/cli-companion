//! Contratto `UserSkills` di Claude Code: le skill personali stanno in
//! `<cartella di configurazione>/skills` (`~/.claude/skills` o
//! `CLAUDE_CONFIG_DIR`), anche nella home Linux delle distro WSL.

use super::config_dir::windows_claude_dir;
use super::ClaudeCode;
use crate::agents::UserSkills;
use std::path::PathBuf;

/// Cartella delle skill dentro la cartella di configurazione.
const SKILLS_DIR_NAME: &str = "skills";

/// Cartella delle skill dentro la home Linux di una distro.
const WSL_SKILLS_SUBPATH: &str = ".claude/skills";

impl UserSkills for ClaudeCode {
    /// `CLAUDE_CONFIG_DIR` o `~/.claude`, come il resto dell'integrazione.
    fn windows_dir(&self) -> Result<PathBuf, String> {
        windows_claude_dir().map(|dir| dir.join(SKILLS_DIR_NAME))
    }

    fn wsl_home_subpath(&self) -> &'static str {
        WSL_SKILLS_SUBPATH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_live_in_the_skills_folder_of_the_config_dir() {
        if let Ok(dir) = ClaudeCode.windows_dir() {
            assert!(dir.ends_with(SKILLS_DIR_NAME), "{dir:?}");
        }
    }

    #[test]
    fn distro_skills_live_in_the_dot_claude_folder_of_the_home() {
        assert_eq!(ClaudeCode.wsl_home_subpath(), ".claude/skills");
    }
}
