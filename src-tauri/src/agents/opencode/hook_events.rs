//! Contratto `HookEvents` di OpenCode: legge il JSON che il plugin passa su
//! stdin a `cli-companion --notify` e scrive la decisione di blocco di fine turno.
//!
//! Chiavi dello stdin (stesse del plugin): `session_id`, `source`,
//! `continuing`, `wait`, `message`, `tool`, `summary`, `reason`.

use super::OpenCode;
use crate::agents::{HookEvent, HookEventKind, HookEvents, WaitKind};
use serde_json::Value;

/// Valori di `wait` scritti dal plugin (sono i nomi neutri del core).
const WAIT_PERMISSION: &str = "permission";
const WAIT_QUESTION: &str = "question";
const WAIT_IDLE: &str = "idle";

/// Chiave della decisione di blocco letta dal plugin sull'ultima riga di stdout.
const CONTINUE_KEY: &str = "continue";

/// Tetti sui campi dello stdin.
const MESSAGE_MAX_CHARS: usize = 300;
const FIELD_MAX_CHARS: usize = 128;
const TOOL_SUMMARY_MAX_CHARS: usize = 200;

impl HookEvents for OpenCode {
    fn normalize(&self, kind: HookEventKind, raw: &str) -> HookEvent {
        parse_hook_stdin(kind, raw)
    }

    /// `{"continue":"<testo>"}`: il plugin lo invia come nuovo prompt e il turno prosegue.
    fn stop_block_output(&self, reason: &str) -> String {
        serde_json::json!({ CONTINUE_KEY: reason }).to_string()
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

/// Tipo neutro di un `wait`; uno sconosciuto e' un'attesa generica.
fn wait_kind(wait: &str) -> WaitKind {
    match wait {
        WAIT_PERMISSION => WaitKind::Permission,
        WAIT_QUESTION => WaitKind::Question,
        WAIT_IDLE => WaitKind::Idle,
        _ => WaitKind::Other,
    }
}

/// Estrae i campi noti dal JSON del plugin. Tollerante: JSON invalido, non
/// oggetto o campi di tipo sbagliato danno semplicemente `None`.
fn parse_hook_stdin(kind: HookEventKind, raw: &str) -> HookEvent {
    let Ok(v) = serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}')) else {
        return HookEvent::default();
    };
    let field = |k: &str, max: usize| v.get(k).and_then(Value::as_str).and_then(|s| clip(s, max));
    HookEvent {
        agent_session_id: field("session_id", FIELD_MAX_CHARS),
        wait: field("wait", FIELD_MAX_CHARS).filter(|_| kind == HookEventKind::NeedsInput).map(|w| wait_kind(&w)),
        message: field("message", MESSAGE_MAX_CHARS),
        already_continuing: v.get("continuing").and_then(Value::as_bool),
        start_source: field("source", FIELD_MAX_CHARS),
        end_reason: field("reason", FIELD_MAX_CHARS),
        tool_name: field("tool", FIELD_MAX_CHARS),
        tool_summary: field("summary", TOOL_SUMMARY_MAX_CHARS),
        ..HookEvent::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(kind: HookEventKind, raw: &str) -> HookEvent {
        OpenCode.normalize(kind, raw)
    }

    #[test]
    fn session_start_carries_the_source() {
        for source in ["startup", "clear", "resume"] {
            let raw = format!(r#"{{"session_id":"ses_2f1c","source":"{source}"}}"#);
            let e = parse(HookEventKind::SessionStart, &raw);
            assert_eq!(e.agent_session_id.as_deref(), Some("ses_2f1c"));
            assert_eq!(e.start_source.as_deref(), Some(source));
            assert_eq!((e.wait, e.already_continuing, e.end_reason), (None, None, None));
        }
    }

    #[test]
    fn finished_tells_if_the_session_was_already_continuing() {
        for continuing in [true, false] {
            let raw = format!(r#"{{"session_id":"ses_1","continuing":{continuing}}}"#);
            let e = parse(HookEventKind::Finished, &raw);
            assert_eq!(e.already_continuing, Some(continuing));
            assert_eq!(e.agent_session_id.as_deref(), Some("ses_1"));
        }
    }

    #[test]
    fn needs_input_maps_each_wait_to_the_neutral_kind() {
        let wait =
            |w: &str| parse(HookEventKind::NeedsInput, &format!(r#"{{"session_id":"ses_1","wait":"{w}"}}"#)).wait;
        assert_eq!(wait("permission"), Some(WaitKind::Permission));
        assert_eq!(wait("question"), Some(WaitKind::Question));
        assert_eq!(wait("idle"), Some(WaitKind::Idle));
        assert_eq!(wait("other"), Some(WaitKind::Other));
        assert_eq!(wait("something-new"), Some(WaitKind::Other));
        assert_eq!(parse(HookEventKind::NeedsInput, r#"{"session_id":"ses_1"}"#).wait, None);
    }

    #[test]
    fn needs_input_for_a_permission_carries_tool_and_summary() {
        let raw = r#"{"session_id":"ses_9","wait":"permission","tool":"shell","summary":"git status --short"}"#;
        let e = parse(HookEventKind::NeedsInput, raw);
        assert_eq!(e.wait, Some(WaitKind::Permission));
        assert_eq!(e.tool_name.as_deref(), Some("shell"));
        assert_eq!(e.tool_summary.as_deref(), Some("git status --short"));
    }

    #[test]
    fn needs_input_for_a_question_or_a_failure_carries_the_message() {
        let q = parse(HookEventKind::NeedsInput, r#"{"session_id":"s","wait":"question","message":"Which branch?"}"#);
        assert_eq!((q.wait, q.message.as_deref()), (Some(WaitKind::Question), Some("Which branch?")));
        let f = parse(HookEventKind::NeedsInput, r#"{"session_id":"s","wait":"other","message":"Rate limited"}"#);
        assert_eq!((f.wait, f.message.as_deref()), (Some(WaitKind::Other), Some("Rate limited")));
    }

    #[test]
    fn wait_is_only_read_for_needs_input() {
        for kind in [
            HookEventKind::SessionStart,
            HookEventKind::Finished,
            HookEventKind::SessionEnd,
            HookEventKind::PermissionRequest,
        ] {
            assert_eq!(parse(kind, r#"{"wait":"permission"}"#).wait, None, "{kind:?}");
        }
    }

    #[test]
    fn permission_request_carries_tool_and_summary() {
        let e = parse(HookEventKind::PermissionRequest, r#"{"session_id":"ses_3","tool":"edit","summary":"src/a.rs"}"#);
        assert_eq!(e.tool_name.as_deref(), Some("edit"));
        assert_eq!(e.tool_summary.as_deref(), Some("src/a.rs"));
    }

    #[test]
    fn session_end_carries_the_reason() {
        let e = parse(HookEventKind::SessionEnd, r#"{"session_id":"ses_3","reason":"deleted"}"#);
        assert_eq!((e.agent_session_id.as_deref(), e.end_reason.as_deref()), (Some("ses_3"), Some("deleted")));
    }

    #[test]
    fn malformed_input_gives_an_event_without_details() {
        for raw in ["", "not json", "[]", "{}", "null", "\"x\"", "42"] {
            assert_eq!(parse(HookEventKind::NeedsInput, raw), HookEvent::default(), "{raw:?}");
        }
    }

    #[test]
    fn values_of_the_wrong_type_or_blank_are_ignored() {
        let raw = r#"{"session_id":7,"wait":false,"message":["x"],"continuing":"yes","source":"  ","tool":null,"summary":{},"reason":""}"#;
        assert_eq!(parse(HookEventKind::NeedsInput, raw), HookEvent::default());
    }

    #[test]
    fn a_bom_is_tolerated_and_fields_are_trimmed_and_clipped() {
        let long = "x".repeat(1000);
        let raw = format!(
            "\u{feff}{{\"session_id\":\"  s1 \",\"message\":\"{long}\",\"summary\":\"{long}\",\"tool\":\"{long}\"}}"
        );
        let e = parse(HookEventKind::NeedsInput, &raw);
        assert_eq!(e.agent_session_id.as_deref(), Some("s1"));
        assert_eq!(e.message.unwrap().chars().count(), MESSAGE_MAX_CHARS);
        assert_eq!(e.tool_summary.unwrap().chars().count(), TOOL_SUMMARY_MAX_CHARS);
        assert_eq!(e.tool_name.unwrap().chars().count(), FIELD_MAX_CHARS);
    }

    #[test]
    fn the_block_output_is_the_json_the_plugin_reads() {
        let out = OpenCode.stop_block_output("Keep going: \"quote\"\nnext");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["continue"], "Keep going: \"quote\"\nnext");
        assert!(!out.contains('\n'), "una sola riga: il plugin legge l'ultima");
    }
}
