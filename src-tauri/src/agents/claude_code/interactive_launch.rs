//! Contratto `InteractiveLauncher` di Claude Code:
//! `claude --dangerously-skip-permissions [--model <m>] [--effort <livello>] ["/<skill>"]`.
//! Claude Code si configura solo con i flag: nessuna variabile d'ambiente.

use super::presets::plugin_skill_invocation;
use super::{ClaudeCode, INTERACTIVE_COMMAND};
use crate::agents::{
    is_safe_launch_token, EffortLevels, InteractiveLaunch, InteractiveLauncher, InteractiveRequest, LaunchSkill,
};

/// Flag della CLI che sceglie il modello.
const MODEL_FLAG: &str = "--model";

/// Il prompt che avvia la skill: `/<plugin>:<id>` per quelle del plugin,
/// `/<nome>` per quelle dell'utente. `None` se il nome non e' sicuro.
fn skill_invocation(skill: LaunchSkill) -> Option<String> {
    match skill {
        LaunchSkill::Plugin(id) if is_safe_launch_token(id) => Some(plugin_skill_invocation(id)),
        LaunchSkill::User(name) if is_safe_launch_token(name) => Some(format!("/{name}")),
        _ => None,
    }
}

impl InteractiveLauncher for ClaudeCode {
    /// Il prompt della skill va in coda, fra doppi apici (come i preset);
    /// `restore_command` e' la stessa riga senza di lui.
    fn launch(&self, req: &InteractiveRequest) -> Option<InteractiveLaunch> {
        let mut restore_command = INTERACTIVE_COMMAND.to_string();
        if let Some(model) = req.model {
            if !is_safe_launch_token(model) {
                return None;
            }
            restore_command.push_str(&format!(" {MODEL_FLAG} {model}"));
        }
        if let Some(effort) = req.effort {
            restore_command.push(' ');
            restore_command.push_str(&self.command_args(effort)?);
        }
        let command = match req.skill {
            Some(skill) => format!("{restore_command} \"{}\"", skill_invocation(skill)?),
            None => restore_command.clone(),
        };
        Some(InteractiveLaunch { command, restore_command, env: Vec::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentIntegration, Effort};

    const BASE: &str = "claude --dangerously-skip-permissions";

    fn launch(model: Option<&str>, effort: Option<Effort>, skill: Option<LaunchSkill>) -> Option<InteractiveLaunch> {
        ClaudeCode.launch(&InteractiveRequest { model, effort, skill })
    }

    #[test]
    fn an_empty_request_is_the_interactive_command() {
        let l = launch(None, None, None).unwrap();
        assert_eq!(l, InteractiveLaunch { command: BASE.into(), restore_command: BASE.into(), env: vec![] });
        assert_eq!(l.command, ClaudeCode.interactive_command());
    }

    #[test]
    fn model_effort_and_skill_combine_in_a_fixed_order() {
        let plugin = Some(LaunchSkill::Plugin("task-intake"));
        let user = Some(LaunchSkill::User("my-skill"));
        let cases = [
            (Some("opus"), None, None, format!("{BASE} --model opus")),
            (None, Some(Effort::High), None, format!("{BASE} --effort high")),
            (None, None, plugin, format!("{BASE} \"/cli-companion-notify:task-intake\"")),
            (None, None, user, format!("{BASE} \"/my-skill\"")),
            (Some("opus"), Some(Effort::XHigh), None, format!("{BASE} --model opus --effort xhigh")),
            (
                Some("claude-opus-4.1"),
                Some(Effort::Low),
                plugin,
                format!("{BASE} --model claude-opus-4.1 --effort low \"/cli-companion-notify:task-intake\""),
            ),
            (Some("opus"), None, user, format!("{BASE} --model opus \"/my-skill\"")),
        ];
        for (model, effort, skill, expected) in cases {
            let l = launch(model, effort, skill).unwrap();
            assert_eq!(l.command, expected);
            assert!(l.env.is_empty());
        }
    }

    #[test]
    fn restore_keeps_model_and_effort_but_never_the_skill() {
        let l = launch(Some("opus"), Some(Effort::Max), Some(LaunchSkill::User("my-skill"))).unwrap();
        assert_eq!(l.restore_command, format!("{BASE} --model opus --effort max"));
        let l = launch(None, None, Some(LaunchSkill::Plugin("task-intake"))).unwrap();
        assert_eq!(l.restore_command, BASE);
    }

    #[test]
    fn a_plugin_skill_matches_the_preset_invocation() {
        let preset = &crate::agents::presets::PRESETS[0];
        let l = launch(None, None, Some(LaunchSkill::Plugin(preset.id))).unwrap();
        assert_eq!(l.command, crate::agents::PresetLauncher::command(&ClaudeCode, preset));
    }

    #[test]
    fn unsafe_values_are_rejected() {
        let long = "a".repeat(65);
        for bad in ["a b", "a$b", "a\"b", "a%b", "a!b", "a`b", "", long.as_str()] {
            assert!(launch(Some(bad), None, None).is_none(), "model {bad:?}");
            assert!(launch(None, None, Some(LaunchSkill::Plugin(bad))).is_none(), "plugin {bad:?}");
            assert!(launch(None, None, Some(LaunchSkill::User(bad))).is_none(), "user {bad:?}");
        }
    }
}
