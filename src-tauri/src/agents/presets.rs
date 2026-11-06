//! Le skill del plugin: sessioni guidate da una skill (es. Task Intake), che i
//! Bot portano nella loro configurazione (`bots::AgentBot::skill`).
//!
//! Aggiungere una skill = una voce in `PRESETS` + un file in `presets/<id>.md`
//! (SKILL.md completo di frontmatter `name`/`description`, in inglese: lo legge il
//! modello). Il pacchetto di ogni integrazione che dichiara `plugin-skills` le
//! spedisce tutte (vedi il contratto `PluginSkills`).

/// Un preset: id (= nome della skill e della cartella), nome mostrato e SKILL.md.
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub skill: &'static str,
}

impl Preset {
    /// SKILL.md con fine riga LF, qualunque sia il checkout.
    pub fn skill_text(&self) -> String {
        self.skill.replace("\r\n", "\n")
    }

    /// La `description` del frontmatter (vuota se manca: lo esclude un test).
    pub fn description(&self) -> String {
        let text = self.skill_text();
        let front = text.strip_prefix("---\n").and_then(|s| s.split_once("\n---\n")).map_or("", |(front, _)| front);
        front.lines().find_map(|l| l.strip_prefix("description:")).map_or_else(String::new, |d| d.trim().to_string())
    }
}

/// Registro dei preset.
pub static PRESETS: &[Preset] = &[
    Preset { id: "task-intake", name: "Task Intake", skill: include_str!("presets/task-intake.md") },
    Preset { id: "skill-wizard", name: "Skill Wizard", skill: include_str!("presets/skill-wizard.md") },
    Preset { id: "project-setup", name: "Project Setup", skill: include_str!("presets/project-setup.md") },
    Preset { id: "code-answer", name: "Code Answer", skill: include_str!("presets/code-answer.md") },
    Preset { id: "bug-fix", name: "Bug Fix", skill: include_str!("presets/bug-fix.md") },
    Preset { id: "code-research", name: "Code Research", skill: include_str!("presets/code-research.md") },
    Preset { id: "test-qa", name: "Test QA", skill: include_str!("presets/test-qa.md") },
    Preset { id: "docs-writer", name: "Docs Writer", skill: include_str!("presets/docs-writer.md") },
    Preset { id: "safe-refactor", name: "Safe Refactor", skill: include_str!("presets/safe-refactor.md") },
    Preset { id: "git-ops", name: "Git Ops", skill: include_str!("presets/git-ops.md") },
    Preset { id: "release-prep", name: "Release Prep", skill: include_str!("presets/release-prep.md") },
];

#[cfg(test)]
mod tests;
