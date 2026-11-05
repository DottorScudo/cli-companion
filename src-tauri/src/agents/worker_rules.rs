//! Il lato core del contratto `WorkerPermissions`: valida e prepara le regole
//! dei worker passando dall'integrazione che li esegue, senza conoscerne il
//! formato. Un'integrazione che non dichiara il contratto non applica regole.

use super::contracts::{AgentIntegration, RuleFormat};
use super::registry;
use crate::platform::TargetKind;
use crate::validate::{ErrorCode, ValidationError};
use std::path::{Path, PathBuf};

/// Valida e normalizza una lista di regole con il contratto dell'agente
/// `agent_id`. Senza contratto una lista vuota resta vuota e una lista con
/// regole e' rifiutata: non verrebbero mai applicate. Un agente non
/// registrato e' un errore.
pub fn validate_worker_rules(agent_id: &str, rules: &[String]) -> Result<Vec<String>, ValidationError> {
    let integration =
        registry::find(agent_id).map_err(|_| ValidationError::new(ErrorCode::AgentUnknown).value(agent_id))?;
    validate_with(integration, rules)
}

/// Forma delle liste di regole dell'agente `agent_id`; `None` se l'agente non
/// e' registrato o non applica regole.
pub fn worker_rule_format(agent_id: &str) -> Option<RuleFormat> {
    let integration = registry::find(agent_id).ok()?;
    integration.capabilities().worker_permissions.map(|p| p.rule_format())
}

/// File di regole per l'agente `agent_id` del task, nel formato di path del
/// target, con la sola lettura delle cartelle `readable` (vedi
/// `WorkerPermissions::prepare`); `None` = niente da applicare o agente che
/// non applica regole.
pub fn worker_permissions_file(
    agent_id: &str,
    data_dir: &Path,
    workspace_id: &str,
    env_kind: TargetKind,
    allow_deny: (&[String], &[String]),
    readable: &[PathBuf],
) -> Result<Option<String>, String> {
    // Errori letti dal modello via RPC: in inglese.
    let integration = registry::find(agent_id).map_err(|_| format!("unknown agent: {agent_id}"))?;
    prepare_with(integration, data_dir, workspace_id, env_kind, allow_deny, readable)
}

fn validate_with(integration: &dyn AgentIntegration, rules: &[String]) -> Result<Vec<String>, ValidationError> {
    match integration.capabilities().worker_permissions {
        Some(permissions) => permissions.validate_rules(rules),
        None if rules.iter().all(|r| r.trim().is_empty()) => Ok(Vec::new()),
        None => Err(ValidationError::new(ErrorCode::PermissionRulesUnsupported)),
    }
}

fn prepare_with(
    integration: &dyn AgentIntegration,
    data_dir: &Path,
    workspace_id: &str,
    env_kind: TargetKind,
    (allow, deny): (&[String], &[String]),
    readable: &[PathBuf],
) -> Result<Option<String>, String> {
    match integration.capabilities().worker_permissions {
        Some(permissions) => permissions.prepare(data_dir, workspace_id, env_kind, allow, deny, readable),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    fn rules(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rules_are_validated_by_the_contract_of_the_integration() {
        let full = FakeIntegration::full("full");
        assert_eq!(validate_with(&full, &rules(&[" a ", "", "a", "b"])), Ok(rules(&["a", "b"])));
        assert_eq!(validate_with(&full, &rules(&["bad rule"])).unwrap_err().code(), ErrorCode::PermissionRuleBadShape);
    }

    #[test]
    fn an_integration_without_the_contract_applies_no_rules() {
        let bare = FakeIntegration::bare("bare", "bare-cli");
        let data_dir = std::env::temp_dir();
        let allow = rules(&["Bash(git:*)"]);
        let deny = rules(&["Edit"]);
        let readable = [PathBuf::from("hub_attachments")];
        assert_eq!(prepare_with(&bare, &data_dir, "ws-1", TargetKind::Windows, (&allow, &deny), &readable), Ok(None));
        assert_eq!(validate_with(&bare, &[]), Ok(Vec::new()));
        assert_eq!(validate_with(&bare, &rules(&["  ", ""])), Ok(Vec::new()));
        let err = validate_with(&bare, &allow).unwrap_err();
        assert_eq!(err.code(), ErrorCode::PermissionRulesUnsupported);
        assert_eq!(err.message_en(), "the worker agent does not support permission rules");
    }

    #[test]
    fn an_integration_with_the_contract_prepares_its_rules_file() {
        let full = FakeIntegration::full("full");
        let allow = rules(&["a"]);
        assert_eq!(
            prepare_with(&full, &std::env::temp_dir(), "ws-1", TargetKind::Wsl, (&allow, &[]), &[]),
            Ok(Some("rules/ws-1.json".to_string()))
        );
        assert_eq!(prepare_with(&full, &std::env::temp_dir(), "ws-1", TargetKind::Wsl, (&[], &[]), &[]), Ok(None));
        // Le sole cartelle da leggere passano al contratto.
        let readable = [PathBuf::from("hub_attachments")];
        let only_readable = prepare_with(&full, &std::env::temp_dir(), "ws-1", TargetKind::Wsl, (&[], &[]), &readable);
        assert_eq!(only_readable, Ok(Some("rules/ws-1.json".to_string())));
    }

    #[test]
    fn an_unknown_agent_is_an_error_not_a_silent_skip() {
        let result =
            worker_permissions_file("nope", &std::env::temp_dir(), "ws-1", TargetKind::Windows, (&[], &[]), &[]);
        assert_eq!(result, Err("unknown agent: nope".to_string()));
    }

    #[test]
    fn the_default_worker_agent_declares_a_rule_format() {
        let format = worker_rule_format(registry::default_agent().id()).expect("the default agent applies rules");
        assert!(format.max_rules > 0 && format.max_rule_chars > 0 && !format.example.is_empty());
    }

    #[test]
    fn rules_are_validated_per_agent_by_id() {
        let id = registry::default_agent().id();
        assert_eq!(validate_worker_rules(id, &rules(&[" Read ", "Read"])), Ok(rules(&["Read"])));
        let bad = validate_worker_rules(id, &rules(&["bad rule"]));
        assert_eq!(bad.unwrap_err().code(), ErrorCode::PermissionRuleBadShape);
    }

    #[test]
    fn an_unknown_agent_has_no_rule_format_and_rejects_validation() {
        assert_eq!(worker_rule_format("nope"), None);
        let err = validate_worker_rules("nope", &[]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::AgentUnknown);
        assert_eq!(err.message_en(), "unknown agent: nope");
    }
}
