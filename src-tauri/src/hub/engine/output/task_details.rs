//! Un task per intero, per `task_details` (puro): lo stesso JSON del task
//! (`TaskView::to_json`), senza i campi interni (sessione, percorso del
//! worktree, storico delle domande). Se la scheda non sta in `TOOL_RESULT_MAX`
//! i testi e gli elenchi lunghi portano il loro primo pezzo e `more` dice come
//! leggere il resto; con `field` l'esito e' un pezzo di un solo campo (vedi
//! `task_page`). Nulla si perde: ogni esito sta nel limite e ogni campo si
//! rilegge per intero. Specifica, nota, domanda e report li scrive un agente:
//! `untrusted`.

use super::super::task_page::{item_name, item_parts, pointer, TaskPage, LIST_FIELDS, TEXT_FIELDS};
use super::super::turn::TOOL_RESULT_MAX;
use super::super::workspace_names::WorkspaceNames;
use super::{timestamp, without_nulls};
use crate::board::TaskView;
use pieces::{fit, size, Content, Pageable};
use serde_json::{json, Map, Value};

pub(super) mod pieces;
#[cfg(test)]
mod tests;

/// Campi interni del task, che il segretario non deve vedere.
const INTERNAL_FIELDS: [&str; 5] = ["session", "last_report", "questions", "previous_attempt", "can_reorder"];
const TIME_FIELDS: [&str; 4] = ["created_at", "updated_at", "started_at", "finished_at"];
/// Dove sta, in una pagina di un elenco, la voce letta da sola perche' troppo lunga.
const LONE_ITEM: &str = "/items/0";

/// Il task `v` del workspace `workspace` per il modello: la scheda (`page`
/// assente) o il pezzo chiesto di un campo; l'errore e' in inglese, per il modello.
pub fn task_details(
    v: &TaskView,
    workspace: &str,
    names: &WorkspaceNames,
    page: Option<&TaskPage>,
) -> Result<Value, String> {
    let mut card = card(v, workspace, names)?;
    match page {
        None => {
            let pageables: Vec<Pageable> = TEXT_FIELDS.iter().chain(&LIST_FIELDS).map(|f| pageable(f)).collect();
            fit(&mut card, &pageables);
            Ok(card)
        }
        Some(page) => field_page(&card, &v.task.id, page),
    }
}

fn pageable(field: &str) -> Pageable {
    Pageable { field: field.to_string(), pointer: pointer(field) }
}

/// La scheda intera del task, prima di accorciarla.
fn card(v: &TaskView, workspace: &str, names: &WorkspaceNames) -> Result<Value, String> {
    let Value::Object(mut task) = without_nulls(v.to_json(true)) else {
        return Err("The task could not be read.".to_string());
    };
    for field in INTERNAL_FIELDS {
        task.remove(field);
    }
    for field in TIME_FIELDS {
        if let Some(ms) = task.get(field).and_then(Value::as_i64) {
            task.insert(field.to_string(), timestamp(ms));
        }
    }
    shorten_worktree(&mut task);
    shorten_e2e(&mut task, "e2e_run");
    shorten_question(&mut task);
    if let Some(Value::Object(report)) = task.get_mut("report") {
        shorten_e2e(report, "e2e");
    }
    let mut out = Value::Object(task);
    out["workspace"] = Value::from(workspace);
    if let Some(name) = names.name_of(workspace) {
        out["workspace_name"] = Value::from(name);
    }
    out["untrusted"] = Value::Bool(true);
    Ok(out)
}

/// Il pezzo di `page.field` che parte da `page.offset`, grande quanto il
/// limite permette; `next_offset` c'e' solo se il campo continua.
fn field_page(card: &Value, task_id: &str, page: &TaskPage) -> Result<Value, String> {
    let content = card
        .pointer(&pointer(&page.field))
        .and_then(Content::of)
        .ok_or_else(|| format!("This task has no `{}`.", page.field))?;
    let total = content.total();
    if page.offset > 0 && page.offset >= total {
        return Err(format!(
            "`offset` {} is beyond the end of `{}`, which has {total} {}.",
            page.offset,
            page.field,
            content.unit()
        ));
    }
    let mut out = json!({ "task_id": task_id, "field": page.field, "offset": page.offset, "untrusted": true });
    out[content.content_key()] = content.empty();
    out["next_offset"] = Value::from(total);
    out[content.total_key()] = Value::from(total);
    let piece = content.piece(page.offset, TOOL_RESULT_MAX.saturating_sub(size(&out)));
    match content {
        Content::Items(items) if piece.end == page.offset && page.offset < total => {
            out["items"] = json!([items[page.offset]]);
            set_next_offset(&mut out, page.offset + 1, total);
            fit(&mut out, &lone_item_pageables(&page.field, page.offset));
        }
        _ => {
            out[content.content_key()] = piece.value;
            set_next_offset(&mut out, piece.end, total);
        }
    }
    Ok(out)
}

fn set_next_offset(out: &mut Value, end: usize, total: usize) {
    if end < total {
        out["next_offset"] = Value::from(end);
    } else if let Some(fields) = out.as_object_mut() {
        fields.remove("next_offset");
    }
}

/// I testi della voce `index` di `list`, letta da sola perche' non sta in una
/// pagina: con il loro nome, si leggono a pezzi come gli altri campi.
fn lone_item_pageables(list: &str, index: usize) -> Vec<Pageable> {
    match item_parts(list) {
        [] => vec![Pageable { field: item_name(list, index, None), pointer: LONE_ITEM.to_string() }],
        parts => parts
            .iter()
            .map(|part| Pageable { field: item_name(list, index, Some(part)), pointer: format!("{LONE_ITEM}/{part}") })
            .collect(),
    }
}

/// Della domanda del worker in attesa solo id (per `relay_answer`), testo e risposte suggerite.
fn shorten_question(task: &mut Map<String, Value>) {
    if let Some(Value::Object(question)) = task.get_mut("question") {
        question.retain(|key, _| matches!(key.as_str(), "id" | "text" | "options"));
    }
}

/// Del worktree solo branch e base: il percorso e' della macchina dell'utente.
fn shorten_worktree(task: &mut Map<String, Value>) {
    if let Some(Value::Object(worktree)) = task.get_mut("worktree") {
        worktree.retain(|key, _| key == "branch" || key == "base");
    }
}

/// Di un'esecuzione E2E solo l'esito.
fn shorten_e2e(object: &mut Map<String, Value>, field: &str) {
    if let Some(run) = object.get_mut(field) {
        *run = json!({ "status": run["status"] });
    }
}
