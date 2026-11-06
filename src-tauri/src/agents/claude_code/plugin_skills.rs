//! Contratto `PluginSkills` di Claude Code: ogni skill sta nel plugin e si
//! avvia come primo prompt (`/<plugin>:<id>`).

use super::{ClaudeCode, PLUGIN};
use crate::agents::presets::Preset;
use crate::agents::PluginSkills;

/// Path della skill nel pacchetto (usato anche da `plugin.rs`).
pub(super) fn skill_path(skill: &Preset) -> String {
    format!("plugins/{PLUGIN}/skills/{}/SKILL.md", skill.id)
}

/// Il prompt che avvia la skill del plugin `id` (usato dall'avvio dei Bot).
pub(super) fn plugin_skill_invocation(id: &str) -> String {
    format!("/{PLUGIN}:{id}")
}

impl PluginSkills for ClaudeCode {
    fn skill_path(&self, skill: &Preset) -> String {
        skill_path(skill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::presets::PRESETS;

    #[test]
    fn the_invocation_runs_the_plugin_skill() {
        assert_eq!(plugin_skill_invocation(PRESETS[0].id), "/cli-companion-notify:task-intake");
    }

    #[test]
    fn skill_path_is_inside_the_plugin() {
        assert_eq!(ClaudeCode.skill_path(&PRESETS[0]), "plugins/cli-companion-notify/skills/task-intake/SKILL.md");
    }
}
