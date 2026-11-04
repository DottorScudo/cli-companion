//! Registro delle integrazioni: l'unico punto in cui il core nomina i singoli
//! agenti.

use super::claude_code;
use super::contracts::{AgentIntegration, AutomationRequest};
use crate::platform::TargetKind;

/// Integrazioni supportate. Per aggiungerne una: una riga qui. La prima e' quella
/// predefinita dei worker (vedi `default_worker_integration`).
static INTEGRATIONS: &[&dyn AgentIntegration] = &[&claude_code::ClaudeCode];

/// Integrazione che usano i worker quando un task non ne indica una: la prima
/// del registro.
pub fn default_worker_integration() -> &'static dyn AgentIntegration {
    INTEGRATIONS[0]
}

pub fn all() -> &'static [&'static dyn AgentIntegration] {
    INTEGRATIONS
}

pub fn find(id: &str) -> Result<&'static dyn AgentIntegration, String> {
    INTEGRATIONS.iter().find(|i| i.id() == id).copied().ok_or_else(|| format!("agente sconosciuto: {id}"))
}

/// Comando di avvio di una sessione automatica per l'agente `agent_id`.
pub fn automation_command(agent_id: &str, env_kind: TargetKind, req: &AutomationRequest) -> Result<String, String> {
    // Errori letti dal modello via RPC: in inglese.
    let integration = find(agent_id).map_err(|_| format!("unknown agent: {agent_id}"))?;
    integration
        .capabilities()
        .automation
        .and_then(|automation| automation.command(env_kind, req))
        .ok_or_else(|| format!("{} does not support automated sessions", integration.display_name()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::PermissionMode;

    #[test]
    fn registry_ids_are_unique_and_findable() {
        let mut ids: Vec<_> = all().iter().map(|i| i.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
        assert!(find("claude-code").is_ok());
        assert!(find("nope").is_err());
    }

    #[test]
    fn automation_command_reports_unknown_and_unsupported_agents() {
        let req = AutomationRequest {
            bootstrap_prompt: "ok",
            permission_mode: PermissionMode::Default,
            model: None,
            permissions_file: None,
        };
        assert!(automation_command(default_worker_integration().id(), TargetKind::Wsl, &req).is_ok());
        assert_eq!(automation_command("nope", TargetKind::Windows, &req), Err("unknown agent: nope".to_string()));
        // Richiesta non rappresentabile in modo sicuro.
        let bad = AutomationRequest { bootstrap_prompt: "a$b", ..req };
        assert_eq!(
            automation_command(default_worker_integration().id(), TargetKind::Windows, &bad),
            Err("Claude Code does not support automated sessions".to_string())
        );
    }
}
