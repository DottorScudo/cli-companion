//! Validazione dei valori delle impostazioni di un workspace: ogni funzione
//! normalizza (trim, doppioni) e rifiuta con un `ValidationError` tipizzato.
//! La usano sia i comandi dell'interfaccia sia le proposte degli agenti
//! (`settings_patch`), cosi' le regole sono una sola.

use super::model::Launch;
use crate::agents::{BuiltinLaunch, PermissionMode};
use crate::validate::{normalize_repo, valid_model, ErrorCode, RuleList, ValidationError, MODEL_MAX};

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

/// Modello dei worker: trim, vuoto = quello dell'agente.
pub fn normalize_worker_model(model: Option<String>) -> Result<Option<String>, ValidationError> {
    let model = model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    if model.as_deref().is_some_and(|m| !valid_model(m)) {
        return Err(ValidationError::new(ErrorCode::ModelInvalid).max(MODEL_MAX));
    }
    Ok(model)
}

/// Valida una lista di regole di permesso; l'errore porta la lista di appartenenza.
pub fn normalize_permission_rules(rules: &[String], list: RuleList) -> Result<Vec<String>, ValidationError> {
    crate::worker_permissions::validate_rules(rules).map_err(|e| e.in_rules(list))
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
/// uno integrato.
fn valid_default_launch(launches: &[Launch], id: &str, builtin: &[BuiltinLaunch]) -> bool {
    id == SHELL_LAUNCH_ID || launches.iter().any(|l| l.id == id) || builtin.iter().any(|b| b.id == id)
}

/// Come `valid_default_launch`, ma rifiuta con l'errore dell'interfaccia.
pub fn check_default_launch(launches: &[Launch], id: &str, builtin: &[BuiltinLaunch]) -> Result<(), ValidationError> {
    if valid_default_launch(launches, id, builtin) {
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

    #[test]
    fn worker_model_is_trimmed_and_checked() {
        assert_eq!(normalize_worker_model(Some("  opus ".into())).unwrap(), Some("opus".to_string()));
        assert_eq!(normalize_worker_model(Some("  ".into())).unwrap(), None);
        assert_eq!(normalize_worker_model(None).unwrap(), None);
        assert_eq!(code(normalize_worker_model(Some("no good!".into()))), ErrorCode::ModelInvalid);
    }

    #[test]
    fn permission_rules_carry_their_list() {
        let bad = vec!["bad rule".to_string()];
        let e = normalize_permission_rules(&bad, RuleList::Allow).unwrap_err();
        assert!(e.message_it().starts_with("Consentiti: regola"), "{}", e.message_it());
        assert!(e.message_en().starts_with("worker_allow: rule"), "{}", e.message_en());
        let e = normalize_permission_rules(&bad, RuleList::Deny).unwrap_err();
        assert!(e.message_it().starts_with("Vietati: regola"));
        assert_eq!(normalize_permission_rules(&[" Read ".into()], RuleList::Allow).unwrap(), ["Read"]);
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
    fn default_launch_accepts_shell_own_and_builtin_ids() {
        let launches = vec![launch("l1")];
        let builtin = crate::agents::builtin_launch_list();
        for ok in ["shell", "l1", "builtin:claude-code", "builtin:preset:task-intake"] {
            assert!(check_default_launch(&launches, ok, &builtin).is_ok(), "{ok}");
        }
        for bad in ["l2", "builtin:nope", "builtin:orchestrator", ""] {
            assert_eq!(code(check_default_launch(&launches, bad, &builtin)), ErrorCode::DefaultLaunchInvalid, "{bad}");
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
