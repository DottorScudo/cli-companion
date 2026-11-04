//! Contratto `SessionResume` di Claude Code: `claude --resume <id>`.

use super::ClaudeCode;
use crate::agents::{ResumeFlag, ResumeSpec, SessionResume};

/// Nomi con cui si avvia Claude Code; il primo e' quello canonico.
const COMMAND_NAMES: [&str; 2] = ["claude", "claude.exe"];
/// Modello del comando di ripresa: `{command}` e `{id}` sono i segnaposto del
/// contratto (`COMMAND_PLACEHOLDER`, `SESSION_ID_PLACEHOLDER`).
const RESUME_TEMPLATE: &str = "{command} --resume {id}";

impl SessionResume for ClaudeCode {
    fn resume_spec(&self) -> ResumeSpec {
        ResumeSpec {
            command_names: COMMAND_NAMES.to_vec(),
            strip_flags: vec![
                ResumeFlag { names: vec!["--resume", "-r"], takes_value: true },
                ResumeFlag { names: vec!["--continue", "-c"], takes_value: false },
            ],
            resume_template: RESUME_TEMPLATE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentIntegration, COMMAND_PLACEHOLDER, SESSION_ID_PLACEHOLDER};

    #[test]
    fn spec_has_the_expected_data() {
        let spec = ClaudeCode.resume_spec();
        assert_eq!(spec.command_names, vec!["claude", "claude.exe"]);
        assert_eq!(
            spec.strip_flags,
            vec![
                ResumeFlag { names: vec!["--resume", "-r"], takes_value: true },
                ResumeFlag { names: vec!["--continue", "-c"], takes_value: false },
            ]
        );
        assert_eq!(spec.resume_template, "{command} --resume {id}");
    }

    #[test]
    fn template_has_each_placeholder_once() {
        let template = ClaudeCode.resume_spec().resume_template;
        assert_eq!(template.matches(COMMAND_PLACEHOLDER).count(), 1);
        assert_eq!(template.matches(SESSION_ID_PLACEHOLDER).count(), 1);
    }

    #[test]
    fn names_are_lowercase_without_spaces_and_flags_start_with_a_dash() {
        let spec = ClaudeCode.resume_spec();
        for name in &spec.command_names {
            assert!(
                !name.is_empty() && *name == name.to_lowercase() && !name.contains(char::is_whitespace),
                "{name:?}"
            );
        }
        for flag in &spec.strip_flags {
            assert!(!flag.names.is_empty());
            assert!(flag.names.iter().all(|name| name.starts_with('-')), "{flag:?}");
        }
    }

    #[test]
    fn interactive_command_runs_a_declared_program() {
        let interactive = ClaudeCode.interactive_command();
        let program = interactive.split_whitespace().next().unwrap();
        assert!(ClaudeCode.resume_spec().command_names.contains(&program));
    }

    #[test]
    fn spec_serializes_with_the_names_the_frontend_reads() {
        let v = serde_json::to_value(ClaudeCode.resume_spec()).unwrap();
        assert_eq!(v["command_names"], serde_json::json!(["claude", "claude.exe"]));
        assert_eq!(v["strip_flags"][0]["names"], serde_json::json!(["--resume", "-r"]));
        assert_eq!(v["strip_flags"][0]["takes_value"], true);
        assert_eq!(v["strip_flags"][1]["names"], serde_json::json!(["--continue", "-c"]));
        assert_eq!(v["strip_flags"][1]["takes_value"], false);
        assert_eq!(v["resume_template"], "{command} --resume {id}");
    }
}
