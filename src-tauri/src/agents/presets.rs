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
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;

    /// Agent and model names no skill may mention, registered or not: skills read agents and
    /// models from `get_workspace_settings`.
    const FOREIGN_NAMES: &[&str] = &["sonnet", "opus", "haiku", "fable", "claude", "codex", "gemini", "copilot"];

    fn registered_names() -> Vec<String> {
        let mut names = Vec::new();
        for agent in registry::all() {
            names.extend([agent.id(), agent.display_name()].map(str::to_lowercase));
            let models = agent.capabilities().models.map_or(&[][..], |catalog| catalog.suggested());
            names.extend(models.iter().flat_map(|m| [m.id, m.label]).map(str::to_lowercase));
        }
        names
    }

    #[test]
    fn skills_name_no_agent_and_no_model() {
        let mut banned = registered_names();
        banned.extend(FOREIGN_NAMES.iter().map(|name| name.to_string()));
        assert!(banned.len() > FOREIGN_NAMES.len(), "the registry names must be covered");
        for p in PRESETS {
            let skill = p.skill_text().to_lowercase();
            for name in &banned {
                assert!(!skill.contains(name.as_str()), "skill {} names `{name}`", p.id);
            }
        }
    }

    #[test]
    fn registry_is_well_formed() {
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
        for p in PRESETS {
            assert!(p.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{}", p.id);
            let skill = p.skill_text();
            assert!(!skill.contains('\r') && skill.is_ascii(), "{}", p.id);
            let front = skill.strip_prefix("---\n").and_then(|s| s.split_once("\n---\n")).unwrap().0;
            let field = |k: &str| front.lines().find_map(|l| l.strip_prefix(k)).map(str::trim);
            assert_eq!(field("name:"), Some(p.id));
            assert!(field("description:").is_some_and(|d| !d.is_empty() && d.len() <= 1024));
            assert_eq!(field("description:").map(str::to_string), Some(p.description()));
        }
    }

    #[test]
    fn task_intake_skill_covers_the_method() {
        let p = PRESETS.iter().find(|p| p.id == "task-intake").unwrap();
        assert_eq!(p.name, "Task Intake");
        let skill = p.skill_text();
        for t in [
            "propose_tasks",
            "list_tasks",
            "get_task_details",
            "update_task",
            "delete_task",
            "Goal",
            "Context",
            "Constraints",
            "Done when",
            "Verify",
            "depends_on",
            "Task panel",
            "get_workspace_settings",
            "propose_settings",
            "Project Setup",
            "Goal review",
            "kind: \"review\"",
            "all** the tasks",
            "E2E scenarios",
            "playwright.config",
            "CLI_COMPANION_E2E_PORT",
            "e2e-results/junit.xml",
            "open: \"never\"",
            "explicitly asks",
            "repos[...].e2e",
            "`e2e: true`",
            "blocks the\n  delivery",
            "Suite present but not registered",
            "at most one line",
            "Never set `e2e: true`",
            "## Agent and model",
            "`tier: \"standard\"`",
            "`tier: \"strong\"`",
            "Never write a model name from memory",
            "`readiness` is `ready`",
            "the agent of the workspace",
            "a capability that only another agent",
            "different agent",
        ] {
            assert!(skill.contains(t), "{t}");
        }
    }

    #[test]
    fn skill_wizard_skill_covers_the_method() {
        let p = PRESETS.iter().find(|p| p.id == "skill-wizard").unwrap();
        assert_eq!(p.name, "Skill Wizard");
        let skill = p.skill_text();
        for t in [
            "save_skill",
            "list_skills",
            "delete_skill",
            "one question at a time",
            "1024",
            "third person",
            "frontmatter",
            "confirmation",
            "/<name>",
            "all workspaces",
            "restarting",
        ] {
            assert!(skill.contains(t), "{t}");
        }
    }

    #[test]
    fn project_setup_skill_covers_the_method() {
        let p = PRESETS.iter().find(|p| p.id == "project-setup").unwrap();
        assert_eq!(p.name, "Project Setup");
        let skill = p.skill_text();
        for t in [
            "get_workspace_settings",
            "propose_settings",
            "Task panel",
            "container_root",
            "copy_ignored",
            "setup",
            "cleanup",
            "bypass",
            "end your turn",
            "$CONTAINER_WORKTREE",
            "repeatable",
            "single line",
            "one question at a time",
            "before -> after",
            "## Worker agent and per-agent settings",
            "worker_agents",
            "rule_format",
            "max_rule_chars",
            "`readiness` is not `ready`",
            "Never write a\n  model name from memory",
        ] {
            assert!(skill.contains(t), "{t}");
        }
    }

    #[test]
    fn project_setup_skill_covers_the_e2e_suite() {
        let p = PRESETS.iter().find(|p| p.id == "project-setup").unwrap();
        let skill = p.skill_text();
        for t in [
            "## E2E suite",
            "playwright.config",
            "cypress.config",
            "pytest-playwright",
            "JUnit",
            "repos[...].e2e",
            "npx playwright test",
            "timeout_min",
            "CLI_COMPANION_E2E_PORT",
            "propose nothing about E2E",
            "You do not edit",
        ] {
            assert!(skill.contains(t), "{t}");
        }
    }
}
