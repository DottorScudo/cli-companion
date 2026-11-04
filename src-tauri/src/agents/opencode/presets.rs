//! Contratto `PresetLauncher` di OpenCode: ogni preset e' una skill del plugin
//! (registrata a runtime), caricata dal primo prompt.

use super::{OpenCode, INTERACTIVE_COMMAND};
use crate::agents::presets::Preset;
use crate::agents::PresetLauncher;

/// Primo prompt di un preset (inglese: lo legge il modello). Senza caratteri
/// speciali per PowerShell, cmd e bash.
const PRESET_PROMPT: &str = "Use the skill tool to load the skill {id}, then follow its instructions.";
const PRESET_PROMPT_ID_PLACEHOLDER: &str = "{id}";

/// Path della skill del preset nel pacchetto (usato anche da `plugin.rs`).
pub(super) fn skill_path(preset: &Preset) -> String {
    format!("skills/{}/SKILL.md", preset.id)
}

impl PresetLauncher for OpenCode {
    /// Come l'interattivo, con il prompt che carica la skill del preset.
    fn command(&self, preset: &Preset) -> String {
        let prompt = PRESET_PROMPT.replace(PRESET_PROMPT_ID_PLACEHOLDER, preset.id);
        format!("{INTERACTIVE_COMMAND} --prompt \"{prompt}\"")
    }

    fn skill_path(&self, preset: &Preset) -> String {
        skill_path(preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::presets::PRESETS;

    #[test]
    fn preset_command_loads_the_plugin_skill() {
        assert_eq!(
            OpenCode.command(&PRESETS[0]),
            "opencode --standalone --prompt \"Use the skill tool to load the skill task-intake, then follow its instructions.\""
        );
    }

    #[test]
    fn every_preset_gets_its_own_id_in_the_prompt() {
        for preset in PRESETS {
            assert!(OpenCode.command(preset).contains(&format!("skill {}, then", preset.id)));
        }
    }

    #[test]
    fn skill_path_is_inside_the_plugin_folder() {
        assert_eq!(OpenCode.skill_path(&PRESETS[0]), "skills/task-intake/SKILL.md");
    }
}
