//! Contratto `PresetLauncher` di Claude Code: ogni preset e' una skill del
//! plugin, avviata come primo prompt (`/<plugin>:<id>`).

use super::{ClaudeCode, INTERACTIVE_COMMAND, PLUGIN};
use crate::agents::presets::Preset;
use crate::agents::PresetLauncher;

/// Path della skill del preset nel pacchetto (usato anche da `plugin.rs`).
pub(super) fn skill_path(preset: &Preset) -> String {
    format!("plugins/{PLUGIN}/skills/{}/SKILL.md", preset.id)
}

impl PresetLauncher for ClaudeCode {
    /// Come l'interattivo, con la skill del preset come primo prompt.
    fn command(&self, preset: &Preset) -> String {
        format!("{INTERACTIVE_COMMAND} \"/{PLUGIN}:{}\"", preset.id)
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
    fn preset_command_runs_the_plugin_skill() {
        assert_eq!(
            ClaudeCode.command(&PRESETS[0]),
            "claude --dangerously-skip-permissions \"/cli-companion-notify:task-intake\""
        );
    }

    #[test]
    fn skill_path_is_inside_the_plugin() {
        assert_eq!(ClaudeCode.skill_path(&PRESETS[0]), "plugins/cli-companion-notify/skills/task-intake/SKILL.md");
    }
}
