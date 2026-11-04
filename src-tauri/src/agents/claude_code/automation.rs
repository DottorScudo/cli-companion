//! Contratto `WorkerAutomation` di Claude Code.

use super::{ClaudeCode, MCP_ALLOW_RULE};
use crate::agents::{AutomationRequest, PermissionMode, WorkerAutomation};
use crate::platform::TargetKind;

impl WorkerAutomation for ClaudeCode {
    /// `claude "<prompt>" --permission-mode <m> [--settings "<file>"] --allowedTools <regola> [--model <m>]`.
    /// Il prompt va PRIMA dei flag (`--allowedTools` e' variadico). La stringa
    /// e' digitata in PowerShell, cmd o bash/zsh: un prompt con caratteri
    /// speciali per una qualunque di queste shell viene rifiutato.
    fn command(&self, _env_kind: TargetKind, req: &AutomationRequest) -> Option<String> {
        let unsafe_char = |c: char| matches!(c, '"' | '$' | '`' | '%' | '!' | '\\') || c.is_control();
        if req.bootstrap_prompt.is_empty() || req.bootstrap_prompt.chars().any(unsafe_char) {
            return None;
        }
        let mode = match req.permission_mode {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Plan => "plan",
            PermissionMode::Bypass => "bypassPermissions",
        };
        // `--settings` prima di `--allowedTools` (variadico). Il path sta tra
        // doppi apici, valido in PowerShell, cmd e bash se privo di
        // `" $ ` % !` (altrimenti niente comando).
        let settings = match req.permissions_file {
            Some(p) if crate::agents::is_shell_safe_path(p) => format!(" --settings \"{p}\""),
            Some(_) => return None,
            None => String::new(),
        };
        let mut cmd = format!(
            "claude \"{}\" --permission-mode {mode}{settings} --allowedTools {MCP_ALLOW_RULE}",
            req.bootstrap_prompt
        );
        if let Some(m) = req.model {
            let ok = !m.is_empty()
                && m.len() <= 64
                && m.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            if !ok {
                return None;
            }
            cmd.push_str(" --model ");
            cmd.push_str(m);
        }
        Some(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(mode: PermissionMode, model: Option<&'a str>, prompt: &'a str) -> AutomationRequest<'a> {
        AutomationRequest { bootstrap_prompt: prompt, permission_mode: mode, model, permissions_file: None }
    }

    #[test]
    fn automation_command_adds_quoted_settings_before_allowed_tools() {
        let mut r = req(PermissionMode::Default, Some("opus"), "ciao");
        r.permissions_file = Some(r"C:\Users\Mario Rossi\AppData\Roaming\cli-companion\worker-settings\ws-1.json");
        let c = ClaudeCode.command(TargetKind::Windows, &r).unwrap();
        assert_eq!(
            c,
            "claude \"ciao\" --permission-mode default --settings \"C:\\Users\\Mario Rossi\\AppData\\Roaming\\cli-companion\\worker-settings\\ws-1.json\" --allowedTools mcp__plugin_cli-companion-notify_cli-companion --model opus"
        );
        r.permissions_file = Some("/mnt/c/Users/me/AppData/Roaming/cli-companion/worker-settings/ws-1.json");
        let c = ClaudeCode.command(TargetKind::Wsl, &r).unwrap();
        assert!(c.contains(
            " --settings \"/mnt/c/Users/me/AppData/Roaming/cli-companion/worker-settings/ws-1.json\" --allowedTools"
        ));
        // Path con caratteri speciali per la shell: nessun comando.
        for bad in ["C:\\a$b\\x.json", "C:\\a`b", "C:\\100%\\x", "C:\\a\"b", "C:\\hi!\\x"] {
            r.permissions_file = Some(bad);
            assert!(ClaudeCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn automation_command_every_env_and_mode() {
        let p = "Sei una sessione automatica. Chiama get_task.";
        let cases = [
            (PermissionMode::Default, "default"),
            (PermissionMode::AcceptEdits, "acceptEdits"),
            (PermissionMode::Plan, "plan"),
            (PermissionMode::Bypass, "bypassPermissions"),
        ];
        for env in [TargetKind::Windows, TargetKind::Wsl] {
            for (mode, flag) in cases {
                let c = ClaudeCode.command(env, &req(mode, None, p)).unwrap();
                assert_eq!(
                    c,
                    format!(
                        "claude \"{p}\" --permission-mode {flag} --allowedTools mcp__plugin_cli-companion-notify_cli-companion"
                    )
                );
            }
        }
    }

    #[test]
    fn automation_command_model_and_rejections() {
        let c =
            ClaudeCode.command(TargetKind::Wsl, &req(PermissionMode::Plan, Some("claude-opus-4.1"), "ciao")).unwrap();
        assert!(c.ends_with(" --model claude-opus-4.1"));
        for bad in ["a\"b", "a$b", "a`b", "a%b", "a!b", "a\\b", "a\nb", ""] {
            let r = req(PermissionMode::Default, None, bad);
            assert!(ClaudeCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
        let long = "a".repeat(65);
        for bad in ["", "m x", "m;ls", "m\"", long.as_str()] {
            let r = req(PermissionMode::Default, Some(bad), "ok");
            assert!(ClaudeCode.command(TargetKind::Windows, &r).is_none(), "{bad:?}");
        }
        // Tramite l'helper pubblico del registro.
        let r = req(PermissionMode::Default, None, "ok");
        assert!(crate::agents::automation_command("claude-code", TargetKind::Wsl, &r).is_ok());
    }
}
