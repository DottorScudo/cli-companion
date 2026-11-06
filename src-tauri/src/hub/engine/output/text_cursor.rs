//! I cursori dei testi accorciati di un evento (puro): un riassunto o una nota
//! che l'evento porta accorciati diventano, per il modello, la lunghezza intera
//! e la chiamata esatta che li legge per intero (`read_with`). Nell'evento
//! restano solo il pezzo iniziale (vedi `events::snapshot`), il resto e' nel
//! task della board.

use crate::hub::model::HubEvent;
use serde_json::{json, Map, Value};

/// Il tool che legge i testi interi di un task.
const READER_TOOL: &str = "task_details";

/// Un testo dell'evento che si puo' accorciare: la chiave della lunghezza
/// intera nel JSON dell'evento, quella del cursore per il modello e il campo
/// del task (per `task_details`) che tiene il testo intero.
struct Shortenable {
    chars_key: &'static str,
    marker_key: &'static str,
    task_field: &'static str,
}

const SHORTENABLE: [Shortenable; 2] = [
    Shortenable { chars_key: "summary_chars", marker_key: "summary_truncated", task_field: "report.summary" },
    Shortenable { chars_key: "note_chars", marker_key: "note_truncated", task_field: "note" },
];

/// Le chiavi dei cursori, che restano anche quando dell'evento si tengono solo gli id.
pub const MARKER_KEYS: [&str; 2] = [SHORTENABLE[0].marker_key, SHORTENABLE[1].marker_key];

/// Sostituisce, nel JSON `event` dell'evento `source`, le lunghezze dei testi
/// accorciati con i loro cursori. La lettura parte da 0: il pezzo dell'evento
/// e' ripulito dai caratteri di controllo, quindi la sua lunghezza non e' un
/// offset esatto nel testo della board.
pub fn mark_shortened(source: &HubEvent, event: &mut Value) {
    let Some(fields) = event.as_object_mut() else {
        return;
    };
    for text in &SHORTENABLE {
        let Some(total) = fields.remove(text.chars_key).and_then(|v| v.as_u64()) else {
            continue;
        };
        let mut marker = json!({ "total_chars": total });
        if let Some(task_id) = &source.task_id {
            marker["read_with"] = read_with(task_id, source.workspace.as_deref(), text.task_field);
        }
        fields.insert(text.marker_key.to_string(), marker);
    }
}

/// La chiamata a `task_details` che legge `field` del task dall'inizio.
fn read_with(task_id: &str, workspace: Option<&str>, field: &str) -> Value {
    let mut arguments = Map::new();
    arguments.insert("task_id".into(), Value::from(task_id));
    if let Some(workspace) = workspace {
        arguments.insert("workspace".into(), Value::from(workspace));
    }
    arguments.insert("field".into(), Value::from(field));
    arguments.insert("offset".into(), Value::from(0));
    json!({ "tool": READER_TOOL, "arguments": arguments })
}

#[cfg(test)]
mod tests;
