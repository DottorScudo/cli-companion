//! La modifica di un task chiesta da un segretario (`request_task_update`):
//! il diff mostrato nella scheda e il confronto con il task di adesso. La
//! validazione vera (stato, grafo, modello dell'agente) e' di `board::Board::update_task`:
//! qui si ricava solo cosa cambia davvero.

use crate::board::{parse_spec, Task, TaskSpec};
use crate::hub::model::{FieldChange, TaskField, TaskUpdate};
use crate::hub::text::clean_text;
use serde_json::{json, Map, Value};

/// Caratteri di un valore nel diff della scheda; il resto sta nel patch.
pub const PREVIEW_KEPT: usize = 600;
/// I campi che un segretario puo' cambiare, nell'ordine della scheda.
const FIELDS: [TaskField; 4] = [TaskField::Title, TaskField::Task, TaskField::DependsOn, TaskField::Model];
const NOTHING_TO_CHANGE: &str = "The task already has these values: there is nothing to change.";

/// La chiave del campo in `TaskSpec` (e nel patch di `update_task`).
pub fn key_of(field: TaskField) -> &'static str {
    match field {
        TaskField::Title => "title",
        TaskField::Task => "task",
        TaskField::DependsOn => "depends_on",
        TaskField::Model => "model",
    }
}

/// Il valore del campo in `spec`, come lo scrive il patch.
fn value_of(field: TaskField, spec: &TaskSpec) -> Value {
    match field {
        TaskField::Title => json!(spec.title),
        TaskField::Task => json!(spec.task),
        TaskField::DependsOn => json!(spec.depends_on),
        TaskField::Model => json!(spec.model),
    }
}

/// Il valore per la scheda: troncato, `None` se vuoto (nessun modello, nessuna dipendenza).
fn preview(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    };
    Some(clean_text(&text, PREVIEW_KEPT)).filter(|t| !t.is_empty())
}

/// Cosa cambia in `task` applicando `patch` (i campi di `TaskSpec`: titolo,
/// specifica, dipendenze, modello). Gli altri campi si rifiutano. L'errore e'
/// in inglese, per il modello; senza differenze non c'e' nulla da chiedere.
pub fn build(task: &Task, patch: &Value) -> Result<TaskUpdate, String> {
    let allowed_only = patch.as_object().is_some_and(|o| o.keys().all(|k| FIELDS.iter().any(|f| key_of(*f) == k)));
    if !allowed_only {
        return Err("Only `title`, `task`, `depends_on` and `model` can be changed.".to_string());
    }
    let current = TaskSpec::from_task(task);
    let wanted = parse_spec(patch, Some(&current))?;
    let (mut changes, mut new, mut old) = (Vec::new(), Map::new(), Map::new());
    for field in FIELDS {
        let (before, after) = (value_of(field, &current), value_of(field, &wanted));
        if before == after {
            continue;
        }
        changes.push(FieldChange { field, before: preview(&before), after: preview(&after) });
        new.insert(key_of(field).to_string(), after);
        old.insert(key_of(field).to_string(), before);
    }
    if changes.is_empty() {
        return Err(NOTHING_TO_CHANGE.to_string());
    }
    Ok(TaskUpdate { changes, patch: Value::Object(new), base: Value::Object(old) })
}

/// Il task ha ancora i valori che aveva quando la modifica e' stata chiesta.
pub fn base_matches(update: &TaskUpdate, task: &Task) -> bool {
    let current = TaskSpec::from_task(task);
    update.changes.iter().all(|c| update.base.get(key_of(c.field)) == Some(&value_of(c.field, &current)))
}

#[cfg(test)]
mod tests;
