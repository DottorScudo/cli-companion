//! Esiti dei tool accorciati (puro), sempre JSON valido e mai piu' lunghi del
//! limite chiesto, misurato sul testo che il modello riceve: dicono quanto era
//! lungo l'esito intero e ne tengono l'inizio (`head`). Quello di un turno
//! passato dice anche come rileggerlo (`reread`); quello che supera il limite
//! del turno in corso e' un errore esplicito.

use super::super::chat::ToolCall;
use super::task_details::pieces::{size, Content};
use crate::hub::settings::{SecretaryTool, ToolClass};
use serde_json::{json, Value};

/// L'esito `content` di un turno passato in al massimo `max` caratteri: se la
/// chiamata `call` che l'ha prodotto e' una lettura, `reread` la ripete con gli
/// stessi argomenti. Le modifiche non si ripetono mai (`reread` assente).
pub fn shortened(content: &str, call: Option<&ToolCall>, max: usize) -> String {
    let mut out = json!({ "shortened": true, "total_chars": content.chars().count() });
    if let Some(reread) = call.and_then(reread) {
        out["reread"] = reread;
        // Argomenti troppo lunghi per il limite: resta solo l'inizio dell'esito.
        if size(&with_empty_head(&out)) > max {
            if let Some(fields) = out.as_object_mut() {
                fields.remove("reread");
            }
        }
    }
    with_head(out, content, max)
}

/// L'esito `content` di `tool` che supera `max` caratteri, al posto del JSON
/// tagliato: un errore che lo dice, con la lunghezza intera e l'inizio.
pub fn oversized(tool: &str, content: &str, max: usize) -> String {
    let total = content.chars().count();
    let out = json!({
        "error": format!(
            "The result of `{tool}` was too long to return ({total} characters): only its beginning is in `head`. \
            Ask for less, for example a smaller page or a single field."
        ),
        "total_chars": total,
    });
    with_head(out, content, max)
}

/// La chiamata di lettura `call`, da ripetere: `None` per le modifiche e per
/// argomenti che non sono un oggetto JSON.
fn reread(call: &ToolCall) -> Option<Value> {
    let tool = SecretaryTool::ALL.into_iter().find(|t| t.code() == call.name)?;
    if tool.class() != ToolClass::Read {
        return None;
    }
    let arguments = match call.arguments.trim() {
        "" => json!({}),
        raw => serde_json::from_str(raw).ok().filter(Value::is_object)?,
    };
    Some(json!({ "tool": tool.code(), "arguments": arguments }))
}

fn with_empty_head(out: &Value) -> Value {
    let mut out = out.clone();
    out["head"] = Value::from("");
    out
}

/// `out` con `head`, l'inizio piu' lungo di `content` che fa stare tutto in `max` caratteri.
fn with_head(out: Value, content: &str, max: usize) -> String {
    let mut out = with_empty_head(&out);
    let room = max.saturating_sub(size(&out));
    out["head"] = Content::Text(content).piece(0, room).value;
    out.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fake::call;

    const MAX: usize = 600;

    fn awkward(n: usize) -> String {
        "àè\"\\ ok\n\tリ🙂 ".chars().cycle().take(n).collect()
    }

    fn parsed(text: &str) -> Value {
        assert!(text.chars().count() <= MAX, "{} > {MAX}", text.chars().count());
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_shortened_read_says_how_long_it_was_and_repeats_its_call() {
        let content = awkward(5000);
        let read = call("c1", "task_details", r#"{"task_id":"t-1","field":"report.summary","offset":12}"#);
        let v = parsed(&shortened(&content, Some(&read), MAX));
        assert_eq!((v["shortened"].as_bool(), v["total_chars"].as_u64()), (Some(true), Some(5000)));
        assert_eq!(
            v["reread"],
            json!({ "tool": "task_details", "arguments": { "task_id": "t-1", "field": "report.summary", "offset": 12 } })
        );
        let head = v["head"].as_str().unwrap();
        assert!(content.starts_with(head) && head.chars().count() > 300, "{head}");
    }

    #[test]
    fn empty_arguments_reread_with_an_empty_object() {
        let v = parsed(&shortened(&awkward(5000), Some(&call("c", "thread_read", "")), MAX));
        assert_eq!(v["reread"], json!({ "tool": "thread_read", "arguments": {} }));
    }

    #[test]
    fn a_change_or_an_unknown_call_is_never_offered_again() {
        let content = awkward(5000);
        let post = call("c", "post_message", r#"{"body":"ciao"}"#);
        for call in [Some(&post), Some(&call("c", "nope", "{}")), Some(&call("c", "thread_read", "[1]")), None] {
            let v = parsed(&shortened(&content, call, MAX));
            assert!(v.get("reread").is_none() && v["shortened"] == true, "{v}");
        }
    }

    #[test]
    fn arguments_too_long_for_the_limit_are_dropped() {
        let query = format!(r#"{{"text":"{}"}}"#, "q".repeat(MAX));
        let v = parsed(&shortened(&awkward(5000), Some(&call("c", "board_status", &query)), MAX));
        assert!(v.get("reread").is_none(), "{v}");
        assert_eq!(v["total_chars"], 5000);
    }

    #[test]
    fn an_oversized_result_is_a_valid_error_with_its_length_and_beginning() {
        let content = format!(r#"{{"big":"{}"}}"#, awkward(9000));
        let v = parsed(&oversized("thread_read", &content, MAX));
        assert!(v["error"].as_str().unwrap().contains("`thread_read` was too long"), "{v}");
        assert_eq!(v["total_chars"].as_u64(), Some(content.chars().count() as u64));
        assert!(content.starts_with(v["head"].as_str().unwrap()));
    }
}
