//! Validazione dei valori delle impostazioni di un workspace: ogni funzione
//! normalizza (trim, doppioni) e rifiuta con un `ValidationError` tipizzato.
//! La usano sia i comandi dell'interfaccia sia le proposte degli agenti
//! (`settings_patch`), cosi' le regole sono una sola.

use super::model::Launch;
use super::worker_settings::AgentWorkerSettings;
use crate::agents::{model_is_valid, registry, validate_worker_rules, AgentUnavailable, AgentUse, PermissionMode};
use crate::validate::{normalize_repo, ErrorCode, RuleList, ValidationError};

/// Lunghezza massima del nome di un avvio (in caratteri).
pub const MAX_LAUNCH_NAME_LEN: usize = 80;
/// Lunghezza massima del comando di un avvio (in caratteri).
pub const MAX_COMMAND_LEN: usize = 1000;
/// Lunghezza massima delle regole dei worker (in caratteri).
pub const MAX_RULES_LEN: usize = 4000;
/// Limiti di `RepoConfig`: voci per elenco, lunghezza di un comando e di un percorso.
pub const MAX_REPO_COMMANDS: usize = 30;
pub const MAX_REPO_COMMAND_LEN: usize = 2000;
pub const MAX_REPO_PATH_LEN: usize = 300;

/// Id dell'avvio predefinito "shell semplice".
const SHELL_LAUNCH_ID: &str = "shell";
/// Chiave del repository che coincide con la root del workspace.
const ROOT_REPO_KEY: &str = ".";

/// Trim; massimo `MAX_RULES_LEN` caratteri; niente caratteri di controllo
/// a parte a capo e tabulazione.
pub fn normalize_rules(rules: &str) -> Result<String, ValidationError> {
    let r = rules.trim();
    if r.chars().count() > MAX_RULES_LEN {
        return Err(ValidationError::new(ErrorCode::RulesTooLong).max(MAX_RULES_LEN));
    }
    if r.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(ValidationError::new(ErrorCode::RulesControlChars));
    }
    Ok(r.to_string())
}

/// Permesso dei worker da testo (trim): "plan", "default", "accept-edits", "bypass".
pub fn parse_worker_permission(mode: &str) -> Result<PermissionMode, ValidationError> {
    PermissionMode::parse(mode.trim()).ok_or_else(|| ValidationError::new(ErrorCode::PermissionInvalid))
}

/// Agente del workspace: trim, vuoto = il predefinito globale. Una scelta deve
/// essere registrata, abilitata e capace di eseguire i worker.
pub fn normalize_workspace_agent(agent: Option<String>) -> Result<Option<String>, ValidationError> {
    let Some(id) = agent.map(|a| a.trim().to_string()).filter(|a| !a.is_empty()) else { return Ok(None) };
    registry::resolve_agent(Some(&id), None, AgentUse::Worker).map_err(worker_agent_unavailable)?;
    Ok(Some(id))
}

/// L'errore da mostrare quando l'agente scelto non puo' eseguire i worker.
fn worker_agent_unavailable(unavailable: AgentUnavailable) -> ValidationError {
    match unavailable {
        AgentUnavailable::Unknown(id) => ValidationError::new(ErrorCode::AgentUnknown).value(id),
        AgentUnavailable::Disabled { name, .. } => ValidationError::new(ErrorCode::AgentDisabled).value(name),
        AgentUnavailable::MissingContract { name, .. } => {
            ValidationError::new(ErrorCode::AgentWithoutAutomation).value(name)
        }
    }
}

/// Impostazioni dei worker di un agente registrato (anche disabilitato: le
/// sue impostazioni si conservano): modello (trim, vuoto = quello dell'agente)
/// accettato dall'agente e liste di regole nella sua sintassi. Ogni errore
/// porta l'id dell'agente.
pub fn normalize_agent_worker(
    agent_id: &str,
    settings: AgentWorkerSettings,
) -> Result<AgentWorkerSettings, ValidationError> {
    normalize_agent_settings(agent_id, settings).map_err(|e| e.in_agent(agent_id))
}

fn normalize_agent_settings(
    agent_id: &str,
    settings: AgentWorkerSettings,
) -> Result<AgentWorkerSettings, ValidationError> {
    let integration =
        registry::find(agent_id).map_err(|_| ValidationError::new(ErrorCode::AgentUnknown).value(agent_id))?;
    let model = settings.model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    if let Some(model) = model.as_deref().filter(|m| !model_is_valid(integration, m)) {
        return Err(ValidationError::new(ErrorCode::ModelNotForAgent).value(model));
    }
    Ok(AgentWorkerSettings {
        model,
        allow: validate_worker_rules(agent_id, &settings.allow).map_err(|e| e.in_rules(RuleList::Allow))?,
        deny: validate_worker_rules(agent_id, &settings.deny).map_err(|e| e.in_rules(RuleList::Deny))?,
    })
}

/// Trim del comando, troncato a MAX_COMMAND_LEN caratteri (spacing interno
/// preservato: fa parte del comando).
fn sanitize_command(command: &str) -> String {
    command.trim().chars().take(MAX_COMMAND_LEN).collect()
}

/// Valida nome e comando di un avvio: ritorna i valori normalizzati.
pub fn validate_launch(name: &str, command: &str) -> Result<(String, String), ValidationError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ValidationError::new(ErrorCode::LaunchNameMissing));
    }
    if name.chars().count() > MAX_LAUNCH_NAME_LEN {
        return Err(ValidationError::new(ErrorCode::LaunchNameTooLong).max(MAX_LAUNCH_NAME_LEN));
    }
    let command = sanitize_command(command);
    if command.is_empty() {
        return Err(ValidationError::new(ErrorCode::LaunchCommandMissing));
    }
    Ok((name.to_string(), command))
}

/// Id ammesso come predefinito del workspace: "shell", uno degli avvii dati o
/// uno condiviso (`shared`: gli integrati e i Bot abilitati).
fn valid_default_launch(launches: &[Launch], id: &str, shared: &[String]) -> bool {
    id == SHELL_LAUNCH_ID || launches.iter().any(|l| l.id == id) || shared.iter().any(|s| s == id)
}

/// Come `valid_default_launch`, ma rifiuta con l'errore dell'interfaccia.
pub fn check_default_launch(launches: &[Launch], id: &str, shared: &[String]) -> Result<(), ValidationError> {
    if valid_default_launch(launches, id, shared) {
        Ok(())
    } else {
        Err(ValidationError::new(ErrorCode::DefaultLaunchInvalid))
    }
}

/// Chiave di un repository nelle impostazioni: relativa alla root, `/`, "."
/// per la root.
pub fn repo_config_key(repo: &str) -> Result<String, ValidationError> {
    normalize_repo(repo)
        .map(|r| r.unwrap_or_else(|| ROOT_REPO_KEY.to_string()))
        .map_err(|detail| ValidationError::new(ErrorCode::RepoInvalid).value(detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(id: &str) -> Launch {
        Launch { id: id.to_string(), name: id.to_string(), command: "ls".to_string() }
    }

    fn code<T: std::fmt::Debug>(r: Result<T, ValidationError>) -> ErrorCode {
        r.unwrap_err().code()
    }

    #[test]
    fn worker_rules_are_trimmed_and_limited() {
        assert_eq!(normalize_rules("  no tests\nno npm  ").unwrap(), "no tests\nno npm");
        assert_eq!(normalize_rules("   ").unwrap(), "");
        assert!(normalize_rules(&"a".repeat(MAX_RULES_LEN)).is_ok());
        assert_eq!(code(normalize_rules(&"a".repeat(MAX_RULES_LEN + 1))), ErrorCode::RulesTooLong);
        assert_eq!(code(normalize_rules("a\u{0}b")), ErrorCode::RulesControlChars);
        assert!(normalize_rules("tab\there\r\nok").is_ok());
    }

    #[test]
    fn worker_permission_is_parsed_with_trim() {
        assert_eq!(parse_worker_permission(" accept-edits ").unwrap(), PermissionMode::AcceptEdits);
        assert_eq!(code(parse_worker_permission("root")), ErrorCode::PermissionInvalid);
    }

    fn default_agent_id() -> &'static str {
        registry::default_agent().id()
    }

    fn agent_settings(model: Option<&str>, allow: &[&str], deny: &[&str]) -> AgentWorkerSettings {
        let owned = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();
        AgentWorkerSettings { model: model.map(str::to_string), allow: owned(allow), deny: owned(deny) }
    }

    #[test]
    fn the_workspace_agent_is_trimmed_and_empty_means_the_global_default() {
        assert_eq!(normalize_workspace_agent(None), Ok(None));
        assert_eq!(normalize_workspace_agent(Some("  ".into())), Ok(None));
        let id = default_agent_id();
        assert_eq!(normalize_workspace_agent(Some(format!(" {id} "))), Ok(Some(id.to_string())));
    }

    #[test]
    fn an_unknown_workspace_agent_is_rejected_with_its_id() {
        let e = normalize_workspace_agent(Some("nope".into())).unwrap_err();
        assert_eq!(e.code(), ErrorCode::AgentUnknown);
        assert_eq!(e.message_en(), "unknown agent: nope");
        assert_eq!(e.message_it(), "agente sconosciuto: nope");
    }

    #[test]
    fn unavailable_agents_map_to_their_codes_with_the_display_name() {
        let disabled = worker_agent_unavailable(AgentUnavailable::Disabled { id: "x".into(), name: "Fake" });
        assert_eq!(disabled.code(), ErrorCode::AgentDisabled);
        assert_eq!(disabled.message_en(), "Fake is disabled: enable it in the integrations settings");
        let missing = AgentUnavailable::MissingContract { id: "x".into(), name: "Fake", purpose: AgentUse::Worker };
        let missing = worker_agent_unavailable(missing);
        assert_eq!(missing.code(), ErrorCode::AgentWithoutAutomation);
        assert_eq!(missing.message_en(), "Fake does not support automated sessions");
        assert_eq!(worker_agent_unavailable(AgentUnavailable::Unknown("x".into())).code(), ErrorCode::AgentUnknown);
    }

    #[test]
    fn agent_settings_are_trimmed_and_normalized() {
        let id = default_agent_id();
        let out = normalize_agent_worker(id, agent_settings(Some("  "), &[" Read ", "", "Read"], &[" WebFetch"]));
        assert_eq!(out, Ok(agent_settings(None, &["Read"], &["WebFetch"])));
        let kept = normalize_agent_worker(id, agent_settings(Some(" sonnet "), &[], &[])).unwrap();
        assert_eq!(kept.model.as_deref(), Some("sonnet"));
    }

    #[test]
    fn agent_settings_errors_name_the_agent_and_the_list() {
        let id = default_agent_id();
        let prefix = format!("worker_agents['{id}']: ");
        let e = normalize_agent_worker(id, agent_settings(Some("no good!"), &[], &[])).unwrap_err();
        assert_eq!(e.code(), ErrorCode::ModelNotForAgent);
        assert_eq!(e.message_en(), format!("{prefix}model not accepted by the agent: no good!"));
        let e = normalize_agent_worker(id, agent_settings(None, &["bad rule"], &[])).unwrap_err();
        assert!(e.message_en().starts_with(&format!("{prefix}worker_allow: rule")), "{}", e.message_en());
        let e = normalize_agent_worker(id, agent_settings(None, &[], &["bad rule"])).unwrap_err();
        assert!(e.message_it().starts_with(&format!("{prefix}Vietati: regola")), "{}", e.message_it());
    }

    #[test]
    fn agent_settings_need_a_registered_agent() {
        let e = normalize_agent_worker("nope", AgentWorkerSettings::default()).unwrap_err();
        assert_eq!(e.message_en(), "worker_agents['nope']: unknown agent: nope");
    }

    #[test]
    fn launch_is_trimmed_and_requires_fields() {
        assert_eq!(
            validate_launch("  Dev  ", "  npm run dev  ").unwrap(),
            ("Dev".to_string(), "npm run dev".to_string())
        );
        assert_eq!(code(validate_launch("   ", "ls")), ErrorCode::LaunchNameMissing);
        assert_eq!(code(validate_launch("Dev", "   ")), ErrorCode::LaunchCommandMissing);
    }

    #[test]
    fn launch_limits_are_enforced() {
        assert!(validate_launch(&"n".repeat(MAX_LAUNCH_NAME_LEN), "ls").is_ok());
        assert_eq!(code(validate_launch(&"n".repeat(MAX_LAUNCH_NAME_LEN + 1), "ls")), ErrorCode::LaunchNameTooLong);
        let (_, cmd) = validate_launch("Dev", &"x".repeat(MAX_COMMAND_LEN + 500)).unwrap();
        assert_eq!(cmd, "x".repeat(MAX_COMMAND_LEN));
        assert_eq!(MAX_COMMAND_LEN, 1000);
    }

    #[test]
    fn default_launch_accepts_shell_own_and_shared_ids() {
        let launches = vec![launch("l1")];
        let shared = ["builtin:claude-code".to_string(), "bot:intake".to_string()];
        for ok in ["shell", "l1", "builtin:claude-code", "bot:intake"] {
            assert!(check_default_launch(&launches, ok, &shared).is_ok(), "{ok}");
        }
        for bad in ["l2", "builtin:nope", "builtin:orchestrator", "builtin:preset:task-intake", "bot:gone", ""] {
            assert_eq!(code(check_default_launch(&launches, bad, &shared)), ErrorCode::DefaultLaunchInvalid, "{bad}");
        }
    }

    #[test]
    fn repo_keys_are_relative_and_use_slashes() {
        assert_eq!(repo_config_key(" DDB/ ").unwrap(), "DDB");
        assert_eq!(repo_config_key(".").unwrap(), ".");
        assert_eq!(repo_config_key("a\\b").unwrap(), "a/b");
        for bad in ["../x", "/x", ""] {
            assert_eq!(code(repo_config_key(bad)), ErrorCode::RepoInvalid, "{bad}");
        }
    }

    #[test]
    fn repo_errors_keep_the_english_detail_for_mcp() {
        let e = repo_config_key("/abs").unwrap_err();
        assert_eq!(e.message_en(), "repo must be relative to the workspace root: /abs");
        assert_eq!(e.message_it(), "repository non valido: repo must be relative to the workspace root: /abs");
    }
}
