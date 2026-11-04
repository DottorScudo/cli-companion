//! Contratto `WorkerPermissions` di Claude Code: comandi pre-approvati dei
//! worker, cioe' regole `permissions.allow` / `permissions.deny` per workspace,
//! scritte in un file settings passato a `claude --settings <file>` all'avvio
//! del worker.
//!
//! Il file sta in `<app_data>/worker-settings/<workspace_id>.json`. Claude
//! Code unisce le regole di `--settings` a quelle degli altri livelli (le
//! liste `allow`/`deny` si sommano; `deny` vince sempre su `allow`, anche in
//! `bypassPermissions`).

use super::ClaudeCode;
use crate::agents::{is_shell_safe_path, PermissionsGuide, RuleFormat, WorkerPermissions};
use crate::platform::{windows_to_wsl_path, TargetKind};
use crate::validate::{ErrorCode, ValidationError};
use std::path::{Path, PathBuf};

const MAX_RULES: usize = 100;
const MAX_RULE_CHARS: usize = 300;
/// Caratteri di una regola citati in un messaggio di errore.
const CLIPPED_RULE_CHARS: usize = 40;
/// Cartella dei file settings dei worker dentro la cartella dati dell'app.
const SETTINGS_DIR_NAME: &str = "worker-settings";
/// Esempio di regola mostrato al modello.
const RULE_EXAMPLE: &str = "Bash(npm test:*)";

const RULES_HELP: &str = "Una regola per riga, nella sintassi dei permessi di Claude Code: Bash(git:*), Bash(docker exec:*), Edit, WebFetch(domain:example.com). Un prefisso come Bash(docker exec:*) consente qualsiasi comando che inizia così (anche docker exec … mysql): scegli prefissi stretti. Un * in mezzo al comando può non funzionare nelle versioni vecchie di Claude Code. I divieti vincono sempre sui consentiti, ma non sono un confine di sicurezza (lo stesso programma lanciato in altra forma, es. con un path o dentro sh -c, può sfuggire). Valgono per i worker avviati da ora in poi.";

const ALLOW_PLACEHOLDER: &str = "Bash(git:*)\nBash(npm run:*)\nBash(docker exec freud-manager:*)";
const DENY_PLACEHOLDER: &str = "Bash(rm -rf:*)\nBash(git push:*)\nRead(./.env)";

const BYPASS_NOTE: &str = "Claude Code mostra una conferma una tantum, a meno che in ~/.claude/settings.json ci sia skipDangerousModePermissionPrompt: true.";

impl WorkerPermissions for ClaudeCode {
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

    /// Forma ammessa: `Nome` oppure `Nome(...)`, con Nome in
    /// `[A-Za-z0-9_*-]+` (anche `mcp__server__tool`).
    fn validate_rules(&self, rules: &[String]) -> Result<Vec<String>, ValidationError> {
        let mut out: Vec<String> = Vec::new();
        for raw in rules {
            let rule = raw.trim();
            if rule.is_empty() {
                continue;
            }
            check_rule(rule)
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

    /// Scrive (solo se cambia) il file settings del workspace e ritorna il path
    /// per `--settings`, o `None` se non ci sono regole. Il path finisce in una
    /// riga digitata nella shell del tab: se contiene caratteri che nessuna
    /// shell tra PowerShell, cmd e bash tratta in modo letterale tra doppi apici
    /// il worker non parte (errore chiaro invece di un comando rotto).
    fn prepare(
        &self,
        data_dir: &Path,
        workspace_id: &str,
        env_kind: TargetKind,
        allow: &[String],
        deny: &[String],
    ) -> Result<Option<String>, String> {
        if allow.is_empty() && deny.is_empty() {
            return Ok(None);
        }
        let dir = settings_dir(data_dir);
        let file = dir.join(file_name(workspace_id));
        let content = settings_json(allow, deny);
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

fn check_rule(rule: &str) -> Result<(), ErrorCode> {
    if rule.chars().count() > MAX_RULE_CHARS {
        return Err(ErrorCode::PermissionRuleTooLong);
    }
    if rule.chars().any(char::is_control) {
        return Err(ErrorCode::PermissionRuleControlChars);
    }
    let name_len =
        rule.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '*' | '-'))).unwrap_or(rule.len());
    if name_len == 0 {
        return Err(ErrorCode::PermissionRuleBadShape);
    }
    let (name, rest) = rule.split_at(name_len);
    if rest.is_empty() {
        return Ok(());
    }
    if !(rest.starts_with('(') && rest.ends_with(')') && rest.len() > 2) {
        return Err(ErrorCode::PermissionRuleBadShape);
    }
    if name.starts_with("mcp__") {
        return Err(ErrorCode::PermissionRuleMcpParens);
    }
    Ok(())
}

/// Contenuto del file settings per `--settings`.
fn settings_json(allow: &[String], deny: &[String]) -> String {
    let v = serde_json::json!({ "permissions": { "allow": allow, "deny": deny } });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Cartella dei file settings dei worker.
fn settings_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_DIR_NAME)
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

/// Path del file come lo vede `claude`: invariato su Windows, `/mnt/<d>/...`
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

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn validate(rules: &[String]) -> Result<Vec<String>, ValidationError> {
        ClaudeCode.validate_rules(rules)
    }

    fn prepare(
        dir: &Path,
        id: &str,
        kind: TargetKind,
        allow: &[String],
        deny: &[String],
    ) -> Result<Option<String>, String> {
        ClaudeCode.prepare(dir, id, kind, allow, deny)
    }

    #[test]
    fn format_and_guide_are_declared() {
        assert_eq!(
            ClaudeCode.rule_format(),
            RuleFormat { max_rules: MAX_RULES, max_rule_chars: MAX_RULE_CHARS, example: RULE_EXAMPLE }
        );
        let guide = ClaudeCode.guide();
        assert!(guide.rules_help.contains("Bash(git:*)") && !guide.bypass_note.is_empty());
    }

    #[test]
    fn valid_rules_are_trimmed_deduped_and_blank_lines_dropped() {
        let r = validate(&v(&[
            "  Bash(git:*) ",
            "",
            "Bash(git:*)",
            "Edit",
            "WebFetch(domain:example.com)",
            "mcp__server__tool",
            "Bash(docker exec:*)",
        ]))
        .unwrap();
        assert_eq!(
            r,
            v(&["Bash(git:*)", "Edit", "WebFetch(domain:example.com)", "mcp__server__tool", "Bash(docker exec:*)"])
        );
    }

    #[test]
    fn invalid_rules_are_rejected() {
        for bad in [
            "git status",
            "(git)",
            "Bash(",
            "Bash()",
            "Bash(git:*) extra",
            "Bash(git\nstatus)",
            "Bash(git\u{7}x)",
            "mcp__srv__tool(x)",
            "Ba sh",
        ] {
            assert!(validate(&v(&[bad])).is_err(), "{bad:?}");
        }
        let long = format!("Bash({})", "a".repeat(MAX_RULE_CHARS));
        assert!(validate(&[long]).is_err());
        let many: Vec<String> = (0..=MAX_RULES).map(|i| format!("Bash(c{i}:*)")).collect();
        assert!(validate(&many).is_err());
        let ok: Vec<String> = (0..MAX_RULES).map(|i| format!("Bash(c{i}:*)")).collect();
        assert_eq!(validate(&ok).unwrap().len(), MAX_RULES);
    }

    #[test]
    fn rejected_rules_report_a_typed_error() {
        let code = |rule: &str| validate(&v(&[rule])).unwrap_err().code();
        assert_eq!(code("git status"), ErrorCode::PermissionRuleBadShape);
        assert_eq!(code("Bash(git\nstatus)"), ErrorCode::PermissionRuleControlChars);
        assert_eq!(code("mcp__srv__tool(x)"), ErrorCode::PermissionRuleMcpParens);
        assert_eq!(code(&format!("Bash({})", "a".repeat(MAX_RULE_CHARS))), ErrorCode::PermissionRuleTooLong);
        let many: Vec<String> = (0..=MAX_RULES).map(|i| format!("Bash(c{i}:*)")).collect();
        assert_eq!(validate(&many).unwrap_err().code(), ErrorCode::PermissionRulesTooMany);
        let e = validate(&v(&["a b"])).unwrap_err();
        assert_eq!(e.message_it(), "regola \"a b\": forma attesa: Nome oppure Nome(...), es. Bash(git:*)");
        assert_eq!(e.message_en(), "rule \"a b\": expected Name or Name(...), e.g. Bash(git:*)");
    }

    #[test]
    fn long_rules_are_clipped_in_the_message() {
        let long = format!("{}(x)", "a".repeat(MAX_RULE_CHARS));
        let e = validate(&[long]).unwrap_err();
        assert!(
            e.message_en().contains('…')
                && e.message_en().ends_with(&format!("longer than {MAX_RULE_CHARS} characters"))
        );
    }

    #[test]
    fn settings_json_has_permissions_allow_and_deny() {
        let j: serde_json::Value =
            serde_json::from_str(&settings_json(&v(&["Bash(git:*)"]), &v(&["Bash(rm:*)"]))).unwrap();
        assert_eq!(j["permissions"]["allow"], serde_json::json!(["Bash(git:*)"]));
        assert_eq!(j["permissions"]["deny"], serde_json::json!(["Bash(rm:*)"]));
    }

    #[test]
    fn path_is_converted_for_wsl_and_plain_for_windows() {
        let f = Path::new(r"C:\Users\Me\AppData\Roaming\cli-companion\worker-settings\ws-1.json");
        assert_eq!(path_for_env(f, TargetKind::Windows).unwrap(), f.to_string_lossy());
        assert_eq!(
            path_for_env(f, TargetKind::Wsl).unwrap(),
            "/mnt/c/Users/Me/AppData/Roaming/cli-companion/worker-settings/ws-1.json"
        );
    }

    #[test]
    fn workspace_ids_never_escape_the_settings_dir() {
        assert_eq!(file_name("ws-1_a"), "ws-1_a.json");
        assert_eq!(file_name("../x\\y"), "___x_y.json");
    }

    #[test]
    fn prepare_writes_only_on_change_and_skips_when_empty() {
        let dir = std::env::temp_dir().join(format!("cc-rules-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(prepare(&dir, "ws-1", TargetKind::Windows, &[], &[]).unwrap(), None);
        assert!(!dir.exists());
        let allow = v(&["Bash(git:*)"]);
        let p = prepare(&dir, "ws-1", TargetKind::Windows, &allow, &[]).unwrap().unwrap();
        let on_disk = std::fs::read_to_string(&p).unwrap();
        assert_eq!(on_disk, settings_json(&allow, &[]));
        let m1 = std::fs::metadata(&p).unwrap().modified().unwrap();
        prepare(&dir, "ws-1", TargetKind::Windows, &allow, &[]).unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), m1);
        let p2 = prepare(&dir, "ws-1", TargetKind::Windows, &allow, &v(&["Edit"])).unwrap().unwrap();
        assert_eq!(p, p2);
        assert!(std::fs::read_to_string(&p).unwrap().contains("Edit"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
