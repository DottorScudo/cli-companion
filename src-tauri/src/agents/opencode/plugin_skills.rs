//! Contratto `PluginSkills` di OpenCode: ogni skill sta nel plugin (registrata
//! a runtime) e si carica dal primo prompt.

use super::{OpenCode, INTERACTIVE_COMMAND};
use crate::agents::presets::Preset;
use crate::agents::PluginSkills;

/// Primo prompt di una sessione con una skill (inglese: lo legge il modello).
/// Senza caratteri speciali per PowerShell, cmd e bash.
const SKILL_PROMPT: &str = "Use the skill tool to load the skill {id}, then follow its instructions.";
const SKILL_PROMPT_ID_PLACEHOLDER: &str = "{id}";

/// Path della skill nel pacchetto (usato anche da `plugin.rs`).
pub(super) fn skill_path(skill: &Preset) -> String {
    format!("skills/{}/SKILL.md", skill.id)
}

/// Come l'interattivo, con il prompt che carica la skill `id`: la riga dei Bot
/// con una skill. `id` va gia' verificato dal chiamante.
pub(super) fn skill_command(id: &str) -> String {
    let prompt = SKILL_PROMPT.replace(SKILL_PROMPT_ID_PLACEHOLDER, id);
    format!("{INTERACTIVE_COMMAND} --prompt \"{prompt}\"")
}

impl PluginSkills for OpenCode {
    fn skill_path(&self, skill: &Preset) -> String {
        skill_path(skill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::presets::PRESETS;

    #[test]
    fn the_skill_command_loads_the_plugin_skill() {
        assert_eq!(
            skill_command(PRESETS[0].id),
            "opencode --standalone --prompt \"Use the skill tool to load the skill task-intake, then follow its instructions.\""
        );
    }

    #[test]
    fn every_skill_gets_its_own_id_in_the_prompt() {
        for skill in PRESETS {
            assert!(skill_command(skill.id).contains(&format!("skill {}, then", skill.id)));
        }
    }

    #[test]
    fn skill_path_is_inside_the_plugin_folder() {
        assert_eq!(OpenCode.skill_path(&PRESETS[0]), "skills/task-intake/SKILL.md");
    }
}
