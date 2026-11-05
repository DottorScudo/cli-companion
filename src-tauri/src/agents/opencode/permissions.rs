//! Contratto `WorkerPermissions` di OpenCode: comandi pre-approvati dei worker,
//! cioe' regole `allow` / `deny` per workspace, scritte in un file di
//! configurazione passato con `OPENCODE_CONFIG` all'avvio del worker.
//!
//! Il file sta in `<app_data>/worker-permissions/opencode/<workspace_id>.json`
//! e mette le regole sotto `agents.build.permissions` e `agents.plan.permissions`:
//! quelle di un agente seguono tutte le regole di primo livello (anche quelle
//! di modalita'), e l'ultima che corrisponde vince. Prima i Consentiti, poi i
//! Vietati: un divieto batte un permesso uguale, anche con `--auto`.
//!
//! Le cartelle `readable` diventano una `external_directory` consentita prima
//! di tutte e una `edit` vietata dopo tutte: sola lettura (vedi `readable_dirs`).

use super::mode_rules::{rule, AGENT_BUILD, AGENT_PLAN, ANY_RESOURCE, EFFECT_ALLOW, EFFECT_DENY};
use super::readable_dirs;
use super::{OpenCode, AGENT_ID};
use crate::agents::{is_shell_safe_path, PermissionsGuide, RuleFormat, WorkerPermissions};
use crate::platform::{windows_to_wsl_path, TargetKind};
use crate::validate::{ErrorCode, ValidationError};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_RULES: usize = 100;
const MAX_RULE_CHARS: usize = 300;
/// Caratteri massimi del nome di un'azione.
const MAX_ACTION_CHARS: usize = 64;
/// Caratteri di una regola citati in un messaggio di errore.
const CLIPPED_RULE_CHARS: usize = 40;
/// Cartella dei file delle regole dentro la cartella dati dell'app.
const PERMISSIONS_DIR_NAME: &str = "worker-permissions";
/// Azione di OpenCode per i percorsi fuori dalla cartella di lavoro.
const ACTION_EXTERNAL_DIRECTORY: &str = "external_directory";
/// Azione di OpenCode per le modifiche ai file.
const ACTION_EDIT: &str = "edit";
/// Esempio di regola mostrato al modello.
const RULE_EXAMPLE: &str = "shell(git status *)";

const RULES_HELP: &str = "Una regola per riga, nella forma azione(risorsa), oppure solo l'azione per indicare qualunque risorsa. Azioni: shell, edit, read, glob, grep, webfetch, websearch, skill, subagent, external_directory e i tool MCP (nome_server_nome_tool). Esempi: shell(git status *), edit(src/*), webfetch(https://docs.rs/*), cli-companion_*. Nella risorsa * sostituisce qualsiasi testo e ? un carattere. L'ultima regola che corrisponde vince: i Vietati vengono dopo i Consentiti, quindi un divieto batte sempre un permesso uguale. Non sono un confine di sicurezza (lo stesso comando lanciato in altra forma può sfuggire a un prefisso). Valgono per i worker avviati da ora in poi.";

const ALLOW_PLACEHOLDER: &str = "shell(git status *)\nshell(npm run *)\nedit(src/*)";
const DENY_PLACEHOLDER: &str = "shell(rm -rf *)\nshell(git push *)\nread(*.env)";

const BYPASS_NOTE: &str = "Con il bypass i worker partono con --auto: OpenCode approva tutto ciò che non è negato in modo esplicito. I Vietati restano validi.";

impl WorkerPermissions for OpenCode {
    fn rule_format(&self) -> RuleFormat {
        RuleFormat { max_rules: MAX_RULES, max_rule_chars: MAX_RULE_CHARS, example: RULE_EXAMPLE }
    }

    fn guide(&self) -> PermissionsGuide {
        PermissionsGuide {
            rules_help: RULES_HELP,
            bypass_note: BYPASS_NOTE,
            allow_placeholder: ALLOW_PLACEHOLDER,
            deny_placeholder: DENY_PLACEHOLDER,
        }
    }

    /// Forma ammessa: `azione` oppure `azione(risorsa)`, vedi `parse_rule`.
    fn validate_rules(&self, rules: &[String]) -> Result<Vec<String>, ValidationError> {
        let mut out: Vec<String> = Vec::new();
        for raw in rules {
            let rule = raw.trim();
            if rule.is_empty() {
                continue;
            }
            parse_rule(rule)
                .map_err(|code| ValidationError::new(code).value(clip_for_error(rule)).max(MAX_RULE_CHARS))?;
            if !out.iter().any(|r| r == rule) {
                out.push(rule.to_string());
            }
        }
        if out.len() > MAX_RULES {
            return Err(ValidationError::new(ErrorCode::PermissionRulesTooMany).max(MAX_RULES));
        }
        Ok(out)
    }

    /// Scrive (solo se cambia) il file delle regole del workspace e ritorna il
    /// path per `OPENCODE_CONFIG`, o `None` se non ci sono regole ne' cartelle
    /// da leggere.
    fn prepare(
        &self,
        data_dir: &Path,
        workspace_id: &str,
        env_kind: TargetKind,
        allow: &[String],
        deny: &[String],
        readable: &[PathBuf],
    ) -> Result<Option<String>, String> {
        let readable = readable_dirs::resources(readable, env_kind);
        if allow.is_empty() && deny.is_empty() && readable.is_empty() {
            return Ok(None);
        }
        let dir = permissions_dir(data_dir);
        let file = dir.join(file_name(workspace_id));
        let content = config_json(allow, deny, &readable)?;
        if std::fs::read_to_string(&file).ok().as_deref() != Some(content.as_str()) {
            std::fs::create_dir_all(&dir).map_err(|e| format!("cartella delle regole dei worker: {e}"))?;
            let tmp = file.with_extension("json.tmp");
            std::fs::write(&tmp, &content)
                .and_then(|_| std::fs::rename(&tmp, &file))
                .map_err(|e| format!("scrittura delle regole dei worker: {e}"))?;
        }
        let path = path_for_env(&file, env_kind)?;
        if !is_shell_safe_path(&path) {
            return Err(format!(
                "the app data path contains characters that are unsafe in a shell command line: {path}"
            ));
        }
        Ok(Some(path))
    }
}

fn clip_for_error(rule: &str) -> String {
    let mut s: String = rule.chars().take(CLIPPED_RULE_CHARS).collect();
    if rule.chars().count() > CLIPPED_RULE_CHARS {
        s.push('…');
    }
    s
}

/// Azione: `[A-Za-z0-9_*?.-]{1,64}`.
fn valid_action(action: &str) -> bool {
    !action.is_empty()
        && action.len() <= MAX_ACTION_CHARS
        && action.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '*' | '?' | '.' | '-'))
}

/// Divide una regola in (azione, risorsa): `azione(risorsa)` oppure `azione`
/// (= qualunque risorsa). La risorsa e' tutto cio' che sta fra la prima `(` e
/// l'ultima `)`, quindi puo' contenere parentesi.
fn parse_rule(rule: &str) -> Result<(&str, &str), ErrorCode> {
    if rule.chars().count() > MAX_RULE_CHARS {
        return Err(ErrorCode::PermissionRuleTooLong);
    }
    if rule.chars().any(char::is_control) {
        return Err(ErrorCode::PermissionRuleControlChars);
    }
    let (action, resource) = match rule.split_once('(') {
        None => (rule, ANY_RESOURCE),
        Some((action, rest)) => {
            let resource = rest.strip_suffix(')').ok_or(ErrorCode::PermissionRuleBadShape)?;
            (action, resource)
        }
    };
    if !valid_action(action) || resource.trim().is_empty() {
        return Err(ErrorCode::PermissionRuleBadShape);
    }
    Ok((action, resource))
}

/// Regole nel formato di OpenCode, con `effect` fisso.
fn effect_rules(rules: &[String], effect: &str) -> Result<Vec<Value>, String> {
    rules
        .iter()
        .map(|r| {
            let (action, resource) = parse_rule(r.trim()).map_err(|_| format!("regola non valida: {r}"))?;
            Ok(rule(action, resource, effect))
        })
        .collect()
}

/// Contenuto del file per `OPENCODE_CONFIG`: l'accesso alle cartelle
/// `readable`, i Consentiti, i Vietati e per ultimo il divieto di modificare
/// quelle cartelle, che quindi nessuna regola del workspace riapre.
fn config_json(allow: &[String], deny: &[String], readable: &[String]) -> Result<String, String> {
    let mut permissions: Vec<Value> =
        readable.iter().map(|dir| rule(ACTION_EXTERNAL_DIRECTORY, dir, EFFECT_ALLOW)).collect();
    permissions.extend(effect_rules(allow, EFFECT_ALLOW)?);
    permissions.extend(effect_rules(deny, EFFECT_DENY)?);
    permissions.extend(readable.iter().map(|dir| rule(ACTION_EDIT, dir, EFFECT_DENY)));
    let per_agent = json!({ "permissions": permissions });
    let config = json!({ "agents": { AGENT_BUILD: per_agent, AGENT_PLAN: per_agent } });
    serde_json::to_string_pretty(&config).map_err(|e| format!("regole dei worker: {e}"))
}

/// Cartella dei file delle regole dei worker di OpenCode.
fn permissions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(PERMISSIONS_DIR_NAME).join(AGENT_ID)
}

/// Nome file sicuro per un id workspace (gli id sono generati dall'app, ma il
/// file finisce in un path: niente separatori).
fn file_name(workspace_id: &str) -> String {
    let safe: String = workspace_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect();
    format!("{safe}.json")
}

/// Path del file come lo vede OpenCode: invariato su Windows, `/mnt/<d>/...`
/// per i workspace WSL.
fn path_for_env(file: &Path, env_kind: TargetKind) -> Result<String, String> {
    let win = file.to_string_lossy().to_string();
    if env_kind == TargetKind::Wsl {
        windows_to_wsl_path(&win).ok_or_else(|| format!("path non convertibile per WSL: {win}"))
    } else {
        Ok(win)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(rules: &[&str]) -> Result<Vec<String>, ValidationError> {
        OpenCode.validate_rules(&rules.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn strings(rules: &[&str]) -> Vec<String> {
        rules.iter().map(|s| s.to_string()).collect()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-oc-perm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rules_are_trimmed_deduplicated_and_blanks_dropped() {
        let ok = v(&["  shell(git status *) ", "", "   ", "shell(git status *)", "edit", "cli-companion_*"]).unwrap();
        assert_eq!(ok, ["shell(git status *)", "edit", "cli-companion_*"]);
    }

    #[test]
    fn good_shapes_are_accepted() {
        for ok in [
            "shell(git status *)",
            "edit(src/*)",
            "webfetch(https://docs.rs/*)",
            "read(*.env)",
            "cli-companion_*",
            "external_directory(C:\\Users\\me\\*)",
            "shell(echo (hi))",
            "a.b-c?",
        ] {
            assert!(v(&[ok]).is_ok(), "{ok}");
        }
    }

    #[test]
    fn bad_shapes_are_rejected_with_a_typed_error() {
        let code = |rule: &str| v(&[rule]).unwrap_err().code();
        for bad in
            ["shell(", "shell()", "shell( )", "shell(x", "(x)", "shell git", "sh ell(x)", "shell(x)y", "a/b", "sh$ell"]
        {
            assert_eq!(code(bad), ErrorCode::PermissionRuleBadShape, "{bad:?}");
        }
        assert_eq!(code("shell(a\nb)"), ErrorCode::PermissionRuleControlChars);
        assert_eq!(code(&format!("shell({})", "x".repeat(MAX_RULE_CHARS))), ErrorCode::PermissionRuleTooLong);
        assert_eq!(code(&format!("{}(x)", "a".repeat(MAX_ACTION_CHARS + 1))), ErrorCode::PermissionRuleBadShape);
    }

    #[test]
    fn too_many_rules_are_rejected() {
        let many: Vec<String> = (0..=MAX_RULES).map(|n| format!("shell(cmd{n})")).collect();
        assert_eq!(OpenCode.validate_rules(&many).unwrap_err().code(), ErrorCode::PermissionRulesTooMany);
        assert!(OpenCode.validate_rules(&many[..MAX_RULES]).is_ok());
    }

    #[test]
    fn the_example_is_a_valid_rule() {
        assert_eq!(v(&[RULE_EXAMPLE]).unwrap(), [RULE_EXAMPLE]);
        assert_eq!(OpenCode.rule_format().example, "shell(git status *)");
    }

    #[test]
    fn rule_is_split_in_action_and_resource() {
        assert_eq!(parse_rule("shell(git status *)"), Ok(("shell", "git status *")));
        assert_eq!(parse_rule("edit"), Ok(("edit", "*")));
        assert_eq!(parse_rule("shell(echo (hi))"), Ok(("shell", "echo (hi)")));
    }

    #[test]
    fn config_has_allows_first_and_denies_last_for_both_agents() {
        let json =
            config_json(&strings(&["shell(git status *)", "edit"]), &strings(&["shell(git log *)"]), &[]).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let expected = serde_json::json!([
            { "action": "shell", "resource": "git status *", "effect": "allow" },
            { "action": "edit", "resource": "*", "effect": "allow" },
            { "action": "shell", "resource": "git log *", "effect": "deny" },
        ]);
        assert_eq!(v["agents"]["build"]["permissions"], expected);
        assert_eq!(v["agents"]["plan"]["permissions"], expected);
        assert_eq!(v["agents"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn no_rules_means_no_file() {
        let dir = temp_dir("none");
        assert_eq!(OpenCode.prepare(&dir, "ws-1", TargetKind::Windows, &[], &[], &[]), Ok(None));
        assert!(!dir.exists());
    }

    #[test]
    fn prepare_writes_the_file_once_and_returns_the_path_of_the_target() {
        let dir = temp_dir("write");
        let allow = strings(&["shell(npm test *)"]);
        let win = OpenCode.prepare(&dir, "ws-1", TargetKind::Windows, &allow, &[], &[]).unwrap().unwrap();
        let file = dir.join("worker-permissions").join("opencode").join("ws-1.json");
        assert_eq!(PathBuf::from(&win), file);
        let first = std::fs::metadata(&file).unwrap().modified().unwrap();
        // Stesse regole: il file non viene riscritto.
        OpenCode.prepare(&dir, "ws-1", TargetKind::Windows, &allow, &[], &[]).unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), first);
        // Regole diverse: riscritto.
        OpenCode.prepare(&dir, "ws-1", TargetKind::Windows, &allow, &strings(&["shell(rm *)"]), &[]).unwrap();
        assert!(std::fs::read_to_string(&file).unwrap().contains("deny"));
        // WSL: /mnt/<drive>/...
        #[cfg(windows)]
        {
            let wsl = OpenCode.prepare(&dir, "ws-1", TargetKind::Wsl, &allow, &[], &[]).unwrap().unwrap();
            assert!(wsl.starts_with("/mnt/"), "{wsl}");
            assert!(wsl.ends_with("/worker-permissions/opencode/ws-1.json"), "{wsl}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_workspace_id_cannot_escape_the_folder() {
        assert_eq!(file_name("ws-1_a"), "ws-1_a.json");
        assert_eq!(file_name("../../etc/x"), "______etc_x.json");
        assert_eq!(file_name("a b\\c:d"), "a_b_c_d.json");
        let dir = temp_dir("escape");
        let path = OpenCode.prepare(&dir, "../x", TargetKind::Windows, &strings(&["edit"]), &[], &[]).unwrap().unwrap();
        assert!(PathBuf::from(path).starts_with(dir.join("worker-permissions").join("opencode")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_data_path_unsafe_for_the_shell_is_an_error() {
        let dir = temp_dir("unsafe$dir");
        let err = OpenCode.prepare(&dir, "ws-1", TargetKind::Windows, &strings(&["edit"]), &[], &[]).unwrap_err();
        assert!(err.contains("unsafe"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_guide_explains_order_and_bypass_in_italian() {
        let g = OpenCode.guide();
        assert!(
            g.rules_help.contains("azione(risorsa)") && g.rules_help.contains("L'ultima regola che corrisponde vince")
        );
        assert!(g.rules_help.contains("i Vietati vengono dopo i Consentiti"));
        assert!(g.bypass_note.contains("--auto") && g.bypass_note.contains("Vietati"));
        for line in g.allow_placeholder.lines().chain(g.deny_placeholder.lines()) {
            assert!(parse_rule(line).is_ok(), "{line}");
        }
    }

    #[test]
    fn readable_folders_are_reachable_but_never_editable() {
        let dir = r"C:\data\hub_attachments\channels\ws-1\*".to_string();
        let json = config_json(&strings(&["edit"]), &[], std::slice::from_ref(&dir)).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let expected = serde_json::json!([
            { "action": "external_directory", "resource": dir, "effect": "allow" },
            { "action": "edit", "resource": "*", "effect": "allow" },
            { "action": "edit", "resource": dir, "effect": "deny" },
        ]);
        assert_eq!(v["agents"]["build"]["permissions"], expected);
        // Senza regole del workspace il file serve comunque, per la lettura.
        let data = temp_dir("readable");
        let channel = PathBuf::from(r"C:\data\hub_attachments\channels\ws-1");
        let path = OpenCode.prepare(&data, "ws-1", TargetKind::Windows, &[], &[], &[channel]).unwrap().unwrap();
        assert!(std::fs::read_to_string(path).unwrap().contains("external_directory"));
        let skipped = OpenCode.prepare(&data, "ws-2", TargetKind::Windows, &[], &[], &[PathBuf::from("rel")]);
        assert_eq!(skipped, Ok(None));
        let _ = std::fs::remove_dir_all(&data);
    }
}
