//! Contratto `HookEvents` di Claude Code: legge il JSON che gli hook passano
//! su stdin e scrive la decisione di blocco dello Stop hook.

use super::ClaudeCode;
use crate::agents::{HookEvent, HookEventKind, HookEvents};
use serde_json::Value;

/// Lunghezza massima del riassunto di `tool_input` (comando, path).
const TOOL_SUMMARY_MAX_CHARS: usize = 200;
/// Tetti sui campi dello stdin.
const MESSAGE_MAX_CHARS: usize = 300;
const FIELD_MAX_CHARS: usize = 128;
const TRANSCRIPT_PATH_MAX_CHARS: usize = 1024;

/// Stati di un background task che lo danno per concluso (minuscolo).
const FINISHED_TASK_STATES: [&str; 8] =
    ["completed", "failed", "killed", "stopped", "cancelled", "canceled", "done", "error"];

impl HookEvents for ClaudeCode {
    fn normalize(&self, _kind: HookEventKind, raw: &str) -> HookEvent {
        parse_hook_stdin(raw)
    }

    /// Formato dello Stop hook: `{"decision":"block","reason":..}` fa proseguire il turno.
    fn stop_block_output(&self, reason: &str) -> String {
        serde_json::json!({ "decision": "block", "reason": reason }).to_string()
    }
}

fn clip(s: &str, max_chars: usize) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(max_chars).collect())
    }
}

/// Riassunto di `tool_input` di un tool: il comando di Bash/PowerShell, il
/// path dei tool di file, l'URL di WebFetch; altrimenti `None`.
fn tool_summary(tool: &str, input: &Value) -> Option<String> {
    let key = match tool {
        "Bash" | "PowerShell" => "command",
        "Edit" | "Write" | "MultiEdit" | "Read" => "file_path",
        "NotebookEdit" => "notebook_path",
        "WebFetch" => "url",
        _ => return None,
    };
    let raw = input.get(key)?.as_str()?;
    // Una riga sola: il comando puo' avere a capo.
    let one_line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    clip(&one_line, TOOL_SUMMARY_MAX_CHARS)
}

/// Lavoro in background ancora attivo nel JSON di uno Stop hook: task con
/// stato non concluso (stato assente o sconosciuto = attivo) piu' i cron di
/// sessione. `None` se nessuno dei due array e' presente.
fn background_work(v: &Value) -> Option<u32> {
    let tasks = v.get("background_tasks").and_then(Value::as_array);
    let crons = v.get("session_crons").and_then(Value::as_array);
    if tasks.is_none() && crons.is_none() {
        return None;
    }
    let active = tasks.map_or(0, |t| {
        t.iter()
            .filter(|task| {
                let status = task.get("status").and_then(Value::as_str).unwrap_or("");
                !FINISHED_TASK_STATES.contains(&status.trim().to_lowercase().as_str())
            })
            .count()
    });
    Some((active + crons.map_or(0, Vec::len)).min(u32::MAX as usize) as u32)
}

/// Estrae i campi noti dal JSON dell'hook. Tollerante: JSON invalido, non
/// oggetto o campi di tipo sbagliato danno semplicemente `None`.
fn parse_hook_stdin(raw: &str) -> HookEvent {
    let Ok(v) = serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}')) else {
        return HookEvent::default();
    };
    let field = |k: &str, max: usize| v.get(k).and_then(|x| x.as_str()).and_then(|s| clip(s, max));
    let tool_name = field("tool_name", FIELD_MAX_CHARS);
    let tool_summary = tool_name.as_deref().and_then(|t| tool_summary(t, v.get("tool_input").unwrap_or(&Value::Null)));
    HookEvent {
        tool_name,
        tool_summary,
        agent_session_id: field("session_id", FIELD_MAX_CHARS),
        notification_type: field("notification_type", FIELD_MAX_CHARS),
        message: field("message", MESSAGE_MAX_CHARS),
        already_continuing: v.get("stop_hook_active").and_then(Value::as_bool),
        background_work: background_work(&v),
        start_source: field("source", FIELD_MAX_CHARS),
        end_reason: field("reason", FIELD_MAX_CHARS),
        transcript_path: field("transcript_path", TRANSCRIPT_PATH_MAX_CHARS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> HookEvent {
        ClaudeCode.normalize(HookEventKind::Finished, raw)
    }

    #[test]
    fn permission_request_stdin_carries_tool_and_summary() {
        let raw = r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash",
            "tool_input":{"command":"docker exec freud-manager\n  composer dump-autoload","description":"x"}}"#;
        let e = ClaudeCode.normalize(HookEventKind::PermissionRequest, raw);
        assert_eq!(e.tool_name.as_deref(), Some("Bash"));
        assert_eq!(e.tool_summary.as_deref(), Some("docker exec freud-manager composer dump-autoload"));
        assert_eq!(e.agent_session_id.as_deref(), Some("s1"));
    }

    #[test]
    fn tool_summary_by_tool_and_clipped() {
        let s = |tool: &str, input: serde_json::Value| tool_summary(tool, &input);
        assert_eq!(s("Edit", serde_json::json!({"file_path":"/a/b.rs"})).as_deref(), Some("/a/b.rs"));
        assert_eq!(s("Write", serde_json::json!({"file_path":"C:\\a.txt"})).as_deref(), Some("C:\\a.txt"));
        assert_eq!(s("WebFetch", serde_json::json!({"url":"https://x.dev"})).as_deref(), Some("https://x.dev"));
        assert_eq!(s("Task", serde_json::json!({"prompt":"x"})), None);
        assert_eq!(s("Bash", serde_json::json!({})), None);
        let long = "x".repeat(500);
        assert_eq!(s("Bash", serde_json::json!({"command": long})).unwrap().chars().count(), TOOL_SUMMARY_MAX_CHARS);
        // Tool senza riassunto: solo il nome; stdin senza tool_name: nessun tool.
        let e = parse(r#"{"tool_name":"Task","tool_input":{}}"#);
        assert_eq!((e.tool_name.as_deref(), e.tool_summary), (Some("Task"), None));
        assert_eq!(parse("{}").tool_name, None);
    }

    #[test]
    fn payload_from_hook_stdin() {
        let raw = r#"{"session_id":"s-1","hook_event_name":"Notification","notification_type":"idle_prompt","message":"  Claude aspetta  ","cwd":"C:/x"}"#;
        let e = parse(raw);
        assert_eq!(e.agent_session_id.as_deref(), Some("s-1"));
        assert_eq!(e.notification_type.as_deref(), Some("idle_prompt"));
        assert_eq!(e.message.as_deref(), Some("Claude aspetta"));
    }

    #[test]
    fn session_start_source_and_session_end_reason_are_extracted() {
        let start = parse(r#"{"session_id":"s","source":"clear"}"#);
        assert_eq!((start.start_source.as_deref(), start.end_reason.as_deref()), (Some("clear"), None));
        let end = parse(r#"{"session_id":"s","reason":" other "}"#);
        assert_eq!((end.start_source.as_deref(), end.end_reason.as_deref()), (None, Some("other")));
        // Tipi sbagliati o vuoti: assenti.
        let bad = parse(r#"{"source":5,"reason":"  "}"#);
        assert_eq!((bad.start_source.as_ref(), bad.end_reason.as_ref()), (None, None));
    }

    #[test]
    fn payload_tolerates_bad_stdin() {
        for raw in ["", "non json", "[1,2]", "null", r#"{"session_id":5,"message":null}"#] {
            let e = parse(raw);
            assert_eq!(e.agent_session_id, None, "{raw}");
            assert_eq!(e.message, None, "{raw}");
        }
        // BOM iniziale tollerato.
        let e = parse("\u{feff}{\"session_id\":\"s\"}");
        assert_eq!(e.agent_session_id.as_deref(), Some("s"));
    }

    #[test]
    fn background_work_counts_active_tasks_and_crons() {
        let n = |raw: &str| parse(raw).background_work;
        assert_eq!(n(r#"{"session_id":"s"}"#), None);
        assert_eq!(n(r#"{"background_tasks":[],"session_crons":[]}"#), Some(0));
        assert_eq!(
            n(r#"{"background_tasks":[{"id":"a","status":"running"},{"id":"b"},{"status":"Pending"}]}"#),
            Some(3)
        );
        assert_eq!(
            n(
                r#"{"background_tasks":[{"status":"completed"},{"status":" FAILED "},{"status":"killed"},{"status":"stopped"},{"status":"cancelled"},{"status":"done"},{"status":"error"}]}"#
            ),
            Some(0)
        );
        assert_eq!(
            n(
                r#"{"background_tasks":[{"status":"in_progress"},{"status":"completed"}],"session_crons":[{"id":"c"},{"id":"d"}]}"#
            ),
            Some(3)
        );
        assert_eq!(n(r#"{"session_crons":[{"id":"c"}]}"#), Some(1));
        // Tipi sbagliati: ignorati.
        assert_eq!(n(r#"{"background_tasks":"x","session_crons":5}"#), None);
        assert_eq!(parse(r#"{"stop_hook_active":true}"#).already_continuing, Some(true));
        assert_eq!(parse(r#"{"stop_hook_active":"si"}"#).already_continuing, None);
    }

    #[test]
    fn transcript_path_is_extracted_clipped_and_optional() {
        let raw = r#"{"session_id":"s","transcript_path":"  C:\\Users\\me\\.claude\\projects\\p\\s.jsonl "}"#;
        assert_eq!(parse(raw).transcript_path.as_deref(), Some(r"C:\Users\me\.claude\projects\p\s.jsonl"));
        // Assente, vuoto o di tipo sbagliato: None.
        for raw in [r#"{"session_id":"s"}"#, r#"{"transcript_path":"  "}"#, r#"{"transcript_path":5}"#] {
            assert_eq!(parse(raw).transcript_path, None, "{raw}");
        }
        let long = format!(r#"{{"transcript_path":"{}"}}"#, "a".repeat(2000));
        assert_eq!(parse(&long).transcript_path.unwrap().chars().count(), TRANSCRIPT_PATH_MAX_CHARS);
    }

    #[test]
    fn long_message_is_truncated() {
        let raw = format!(r#"{{"message":"{}"}}"#, "è".repeat(1000));
        assert_eq!(parse(&raw).message.unwrap().chars().count(), MESSAGE_MAX_CHARS);
    }

    #[test]
    fn stop_block_output_is_the_stop_hook_decision() {
        assert_eq!(ClaudeCode.stop_block_output("x"), r#"{"decision":"block","reason":"x"}"#);
        assert_eq!(ClaudeCode.stop_block_output("say \"hi\""), r#"{"decision":"block","reason":"say \"hi\""}"#);
    }
}
