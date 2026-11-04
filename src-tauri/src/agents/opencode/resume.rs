//! Contratto `SessionResume` di OpenCode: `opencode --standalone --session <id>`.

use super::{OpenCode, STANDALONE_FLAG};
use crate::agents::{ResumeFlag, ResumeSpec, SessionResume};

/// Nomi con cui si avvia OpenCode; il primo e' quello canonico.
const COMMAND_NAMES: [&str; 3] = ["opencode", "opencode.exe", "opencode.cmd"];
/// Modello del comando di ripresa: `{command}` e `{id}` sono i segnaposto del
/// contratto. Sempre `--standalone`, anche se il tab era partito con `opencode`
/// semplice (il flag e' tra quelli tolti, quindi non si duplica).
const RESUME_TEMPLATE: &str = "{command} --standalone --session {id}";

impl SessionResume for OpenCode {
    fn resume_spec(&self) -> ResumeSpec {
        ResumeSpec {
            command_names: COMMAND_NAMES.to_vec(),
            strip_flags: vec![
                ResumeFlag { names: vec![STANDALONE_FLAG], takes_value: false },
                ResumeFlag { names: vec!["--session", "-s"], takes_value: true },
                ResumeFlag { names: vec!["--continue", "-c"], takes_value: false },
                ResumeFlag { names: vec!["--fork"], takes_value: false },
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
        let spec = OpenCode.resume_spec();
        assert_eq!(spec.command_names, vec!["opencode", "opencode.exe", "opencode.cmd"]);
        assert_eq!(
            spec.strip_flags,
            vec![
                ResumeFlag { names: vec!["--standalone"], takes_value: false },
                ResumeFlag { names: vec!["--session", "-s"], takes_value: true },
                ResumeFlag { names: vec!["--continue", "-c"], takes_value: false },
                ResumeFlag { names: vec!["--fork"], takes_value: false },
            ]
        );
        assert_eq!(spec.resume_template, "{command} --standalone --session {id}");
    }

    #[test]
    fn template_has_each_placeholder_once() {
        let template = OpenCode.resume_spec().resume_template;
        assert_eq!(template.matches(COMMAND_PLACEHOLDER).count(), 1);
        assert_eq!(template.matches(SESSION_ID_PLACEHOLDER).count(), 1);
    }

    #[test]
    fn names_are_lowercase_without_spaces_and_flags_start_with_a_dash() {
        let spec = OpenCode.resume_spec();
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
    fn the_standalone_flag_is_stripped_so_the_template_never_duplicates_it() {
        let spec = OpenCode.resume_spec();
        assert!(spec.strip_flags.iter().any(|f| f.names.contains(&STANDALONE_FLAG) && !f.takes_value));
        assert!(spec.resume_template.contains(STANDALONE_FLAG));
    }

    #[test]
    fn interactive_command_runs_a_declared_program() {
        let interactive = OpenCode.interactive_command();
        let program = interactive.split_whitespace().next().unwrap();
        assert!(OpenCode.resume_spec().command_names.contains(&program));
    }

    #[test]
    fn spec_serializes_with_the_names_the_frontend_reads() {
        let v = serde_json::to_value(OpenCode.resume_spec()).unwrap();
        assert_eq!(v["command_names"], serde_json::json!(["opencode", "opencode.exe", "opencode.cmd"]));
        assert_eq!(v["strip_flags"][1]["names"], serde_json::json!(["--session", "-s"]));
        assert_eq!(v["strip_flags"][1]["takes_value"], true);
        assert_eq!(v["resume_template"], "{command} --standalone --session {id}");
    }
}
