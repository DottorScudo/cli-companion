//! Contratto `WorkerAutomation` di OpenCode.
//!
//! Il TUI non ha flag per modello e agente: arrivano dalla configurazione.
//! Il core imposta sul tab due variabili (mai digitate):
//! - `OPENCODE_CONFIG_CONTENT`: JSON inline con modello, agente e regole della modalita';
//! - `OPENCODE_CONFIG`: file con le regole del workspace (vedi `permissions.rs`),
//!   che per precedenza battono quelle della modalita'.

use super::mode_rules::config_content;
use super::{OpenCode, INTERACTIVE_COMMAND};
use crate::agents::{
    is_shell_safe_path, AutomationLaunch, AutomationRequest, ModelCatalog, PermissionMode, WorkerAutomation,
};
use crate::platform::TargetKind;

/// Flag che fa approvare a OpenCode tutto cio' che non e' negato in modo esplicito.
const AUTO_FLAG: &str = "--auto";
/// Anche dell'avvio interattivo dei Bot (`interactive_launch.rs`).
pub(super) const ENV_CONFIG_CONTENT: &str = "OPENCODE_CONFIG_CONTENT";
const ENV_CONFIG_FILE: &str = "OPENCODE_CONFIG";

impl WorkerAutomation for OpenCode {
    /// `opencode --standalone --prompt "<prompt>" [--auto]`. La stringa e'
    /// digitata in PowerShell, cmd o bash/zsh: un prompt con caratteri
    /// speciali per una qualunque di queste shell viene rifiutato.
    fn command(&self, _env_kind: TargetKind, req: &AutomationRequest) -> Option<AutomationLaunch> {
        let unsafe_char = |c: char| matches!(c, '"' | '$' | '`' | '%' | '!' | '\\') || c.is_control();
        if req.bootstrap_prompt.is_empty() || req.bootstrap_prompt.chars().any(unsafe_char) {
            return None;
        }
        // Modello non valido per OpenCode: nessun avvio (mai un ripiego silenzioso).
        let model = match req.model {
            Some(m) if self.is_valid(m) => Some(m),
            Some(_) => return None,
            None => None,
        };
        let mut env = vec![(ENV_CONFIG_CONTENT.to_string(), config_content(req.permission_mode, model))];
        match req.permissions_file {
            Some(p) if is_shell_safe_path(p) => env.push((ENV_CONFIG_FILE.to_string(), p.to_string())),
            Some(_) => return None,
            None => {}
        }
        let mut command = format!("{INTERACTIVE_COMMAND} --prompt \"{}\"", req.bootstrap_prompt);
        if req.permission_mode == PermissionMode::Bypass {
            command.push(' ');
            command.push_str(AUTO_FLAG);
        }
        Some(AutomationLaunch { command, env })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const PROMPT: &str = "You are an automated session. Call get_task.";

    fn req<'a>(mode: PermissionMode, model: Option<&'a str>, prompt: &'a str) -> AutomationRequest<'a> {
        AutomationRequest {
            bootstrap_prompt: prompt,
            permission_mode: mode,
            model,
            permissions_file: None,
            effort: None,
        }
    }

    fn content(launch: &AutomationLaunch) -> Value {
        let (name, value) = &launch.env[0];
        assert_eq!(name, ENV_CONFIG_CONTENT);
        serde_json::from_str(value).unwrap()
    }

    #[test]
    fn every_mode_and_target_has_the_exact_command_and_config() {
        let cases = [
            (PermissionMode::Plan, "plan", "", 11),
            (PermissionMode::Default, "build", "", 11),
            (PermissionMode::AcceptEdits, "build", "", 12),
            (PermissionMode::Bypass, "build", " --auto", 2),
        ];
        for kind in [TargetKind::Windows, TargetKind::Wsl] {
            for (mode, agent, suffix, rules) in cases {
                let launch = OpenCode.command(kind, &req(mode, None, PROMPT)).unwrap();
                assert_eq!(launch.command, format!("opencode --standalone --prompt \"{PROMPT}\"{suffix}"));
                assert_eq!(launch.env.len(), 1, "{mode:?}");
                let config = content(&launch);
                assert_eq!(config["default_agent"], agent, "{mode:?}");
                assert_eq!(config["permissions"].as_array().unwrap().len(), rules, "{mode:?}");
                assert!(config.get("model").is_none());
            }
        }
    }

    #[test]
    fn a_valid_model_goes_into_the_config_not_the_command() {
        let launch =
            OpenCode.command(TargetKind::Windows, &req(PermissionMode::Default, Some("opencode/gpt-5.5"), PROMPT));
        let launch = launch.unwrap();
        assert!(!launch.command.contains("gpt-5.5"));
        assert_eq!(content(&launch)["model"], "opencode/gpt-5.5");
    }

    #[test]
    fn a_model_outside_the_catalog_rule_is_refused() {
        let long = format!("p/{}", "m".repeat(200));
        for bad in ["", "sonnet", "m x", "m;ls", "m\"", "a:b/c", "/x", long.as_str()] {
            let r = req(PermissionMode::Default, Some(bad), PROMPT);
            assert!(OpenCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn unsafe_prompts_are_refused() {
        for bad in ["a\"b", "a$b", "a`b", "a%b", "a!b", "a\\b", "a\nb", "a\tb", ""] {
            let r = req(PermissionMode::Default, None, bad);
            assert!(OpenCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn the_permissions_file_travels_in_the_environment() {
        let mut r = req(PermissionMode::Default, None, PROMPT);
        let windows = r"C:\Users\Mario Rossi\AppData\Roaming\x\worker-permissions\opencode\ws-1.json";
        let wsl = "/mnt/c/Users/me/AppData/Roaming/x/worker-permissions/opencode/ws-1.json";
        for (kind, path) in [(TargetKind::Windows, windows), (TargetKind::Wsl, wsl)] {
            r.permissions_file = Some(path);
            let launch = OpenCode.command(kind, &r).unwrap();
            assert!(!launch.command.contains(path), "il path non va nel comando");
            assert_eq!(launch.env[1], (ENV_CONFIG_FILE.to_string(), path.to_string()));
        }
    }

    #[test]
    fn an_unsafe_permissions_file_is_refused() {
        let mut r = req(PermissionMode::Default, None, PROMPT);
        for bad in ["", "C:\\a$b\\x.json", "C:\\a`b", "C:\\100%\\x", "C:\\a\"b", "C:\\hi!\\x", "C:\\a\nb"] {
            r.permissions_file = Some(bad);
            assert!(OpenCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn env_is_valid_for_the_core() {
        let mut r = req(PermissionMode::AcceptEdits, Some("ollama/llama3:8b"), PROMPT);
        r.permissions_file = Some("C:/data/ws.json");
        let launch = OpenCode.command(TargetKind::Windows, &r).unwrap();
        assert_eq!(crate::agents::first_invalid_env_name(&launch.env), None);
    }
}
