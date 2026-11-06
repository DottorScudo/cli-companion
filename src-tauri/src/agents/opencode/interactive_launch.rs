//! Contratto `InteractiveLauncher` di OpenCode.
//!
//! Il TUI non ha `--model`: il modello passa da `OPENCODE_CONFIG_CONTENT`
//! (impostata dal core sul tab, mai digitata) con il solo modello, cosi' un tab
//! interattivo conserva permessi e agente di partenza dell'utente. La skill e'
//! il primo prompt, come per i preset. L'effort non c'e': OpenCode non dichiara
//! `EffortLevels`, quindi il core lo toglie prima (con un avviso) e qui si ignora.

use super::automation::ENV_CONFIG_CONTENT;
use super::mode_rules::model_only_content;
use super::plugin_skills::skill_command;
use super::{OpenCode, INTERACTIVE_COMMAND};
use crate::agents::{
    is_safe_launch_token, InteractiveLaunch, InteractiveLauncher, InteractiveRequest, LaunchSkill, ModelCatalog,
};

impl InteractiveLauncher for OpenCode {
    /// `opencode --standalone [--prompt "<carica la skill>"]`. `restore_command`
    /// e' sempre l'interattivo: riprende la conversazione, il modello resta
    /// nell'ambiente (impostato una volta sola per il tab).
    fn launch(&self, req: &InteractiveRequest) -> Option<InteractiveLaunch> {
        // Modello non valido per OpenCode: nessun avvio (mai un ripiego silenzioso).
        let env = match req.model {
            Some(model) if self.is_valid(model) => vec![(ENV_CONFIG_CONTENT.to_string(), model_only_content(model))],
            Some(_) => return None,
            None => Vec::new(),
        };
        let command = match req.skill {
            Some(LaunchSkill::Plugin(id) | LaunchSkill::User(id)) if is_safe_launch_token(id) => skill_command(id),
            Some(_) => return None,
            None => INTERACTIVE_COMMAND.to_string(),
        };
        Some(InteractiveLaunch { command, restore_command: INTERACTIVE_COMMAND.to_string(), env })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{first_invalid_env_name, Effort};

    fn launch(model: Option<&str>, effort: Option<Effort>, skill: Option<LaunchSkill>) -> Option<InteractiveLaunch> {
        OpenCode.launch(&InteractiveRequest { model, effort, skill })
    }

    #[test]
    fn an_empty_request_is_the_interactive_command_without_environment() {
        let l = launch(None, None, None).unwrap();
        assert_eq!(
            l,
            InteractiveLaunch {
                command: "opencode --standalone".into(),
                restore_command: "opencode --standalone".into(),
                env: vec![]
            }
        );
    }

    #[test]
    fn the_model_travels_in_the_environment_not_in_the_command() {
        let l = launch(Some("opencode/gpt-5.5"), None, None).unwrap();
        assert_eq!(l.command, "opencode --standalone");
        assert_eq!(l.restore_command, "opencode --standalone");
        assert_eq!(l.env, [("OPENCODE_CONFIG_CONTENT".to_string(), r#"{"model":"opencode/gpt-5.5"}"#.to_string())]);
        assert_eq!(first_invalid_env_name(&l.env), None);
    }

    #[test]
    fn a_skill_is_loaded_by_the_first_prompt_and_never_restored() {
        for skill in [LaunchSkill::Plugin("task-intake"), LaunchSkill::User("task-intake")] {
            let l = launch(None, None, Some(skill)).unwrap();
            assert_eq!(
                l.command,
                "opencode --standalone --prompt \"Use the skill tool to load the skill task-intake, then follow its instructions.\""
            );
            assert_eq!(l.restore_command, "opencode --standalone");
            assert!(l.env.is_empty());
        }
    }

    #[test]
    fn a_plugin_skill_is_loaded_by_the_first_prompt() {
        let preset = &crate::agents::presets::PRESETS[0];
        let l = launch(None, None, Some(LaunchSkill::Plugin(preset.id))).unwrap();
        assert_eq!(l.command, skill_command(preset.id));
    }

    #[test]
    fn model_and_skill_combine() {
        let l = launch(Some("ollama/llama3:8b"), None, Some(LaunchSkill::User("my-skill"))).unwrap();
        assert!(l.command.ends_with("skill my-skill, then follow its instructions.\""));
        assert_eq!(l.env.len(), 1);
    }

    #[test]
    fn effort_is_ignored() {
        let with = launch(Some("opencode/gpt-5.5"), Some(Effort::Max), None).unwrap();
        assert_eq!(Some(with), launch(Some("opencode/gpt-5.5"), None, None));
    }

    #[test]
    fn unsafe_values_are_rejected() {
        let long = "a".repeat(65);
        for bad in ["a b", "a$b", "a\"b", "a%b", "a!b", "a`b", "", long.as_str()] {
            assert!(launch(None, None, Some(LaunchSkill::Plugin(bad))).is_none(), "plugin {bad:?}");
            assert!(launch(None, None, Some(LaunchSkill::User(bad))).is_none(), "user {bad:?}");
        }
        for bad in ["sonnet", "a b/c", "p/m;ls", "p/m\"", ""] {
            assert!(launch(Some(bad), None, None).is_none(), "model {bad:?}");
        }
    }
}
