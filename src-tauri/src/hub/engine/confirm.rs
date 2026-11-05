//! Le azioni irreversibili chieste da un segretario (puro, generico sui dati,
//! con l'orologio passato dal chiamante). Un segretario non le esegue mai:
//! `request_task_action` mette nel thread una scheda (`HubActionRequest`) e
//! solo il clic dell'utente su «Conferma» la esegue (`resolve`).
//!
//! - **Apertura** (`open_request`): il thread deve essere quello del workspace
//!   della board, il task deve essere fra i suoi task e sulla board, l'azione
//!   deve riuscire adesso e non ci puo' essere un'altra richiesta aperta per
//!   lo stesso task. La richiesta dura `REQUEST_TTL_MS`.
//! - **Decisione** (`decide`): la conferma vale solo se la richiesta e' in
//!   attesa, non scaduta e il task e' ancora com'era (stesso stato, azione
//!   ancora consentita). Altrimenti non si esegue nulla.
//! - **Invalidazione** (`invalidate_for_task`): quando il task cambia stato o
//!   sparisce, le sue richieste in attesa diventano `stale` senza aspettare il clic.
//!
//! Gli errori di `open_request` sono in inglese (tornano al modello), i testi
//! per l'utente sono in `texts`.

mod resolve;
pub mod texts;

pub use resolve::{resolve, ActionBoard};

use super::board_ops::describe;
use crate::board::{BoardView, TaskAction, TaskView};
use crate::hub::channel::Channel;
use crate::hub::model::{
    ActionRequestState, Author, HubActionRequest, HubEvent, HubEventKind, Message, Thread, ThreadSummary,
};
use crate::hub::state::MESSAGE_ID_PREFIX;
use crate::hub::text::clean_line;
use crate::hub::threads::{summary, ChannelThreads, NewMessage, THREAD_NOT_FOUND};
use serde_json::{json, Value};

/// Durata di una richiesta senza decisione.
pub const REQUEST_TTL_MS: i64 = 30 * 60 * 1000;
const MS_PER_MINUTE: i64 = 60_000;
/// Caratteri del motivo scritto dal segretario.
pub const REASON_MAX: usize = 300;
/// Le azioni che un segretario puo' chiedere.
pub const REQUESTABLE: [TaskAction; 4] =
    [TaskAction::Stop, TaskAction::Delete, TaskAction::DeleteCascade, TaskAction::MarkDone];
/// Caratteri tenuti di un titolo o di un id nella scheda e negli errori.
const TITLE_KEPT: usize = 120;
/// Il thread non e' di un workspace: nel Generale non ci sono task su cui agire.
pub const NOT_A_WORKSPACE_THREAD: &str =
    "Action requests work only in a thread of a workspace channel, for the tasks of that thread.";
const CARD_NOT_POSTED: &str = "The request could not be posted.";

/// Il nome dell'azione sul filo, come nei tool e negli esiti per il modello.
pub fn action_code(action: TaskAction) -> &'static str {
    match action {
        TaskAction::Stop => "stop",
        TaskAction::Delete => "delete",
        TaskAction::DeleteCascade => "delete_cascade",
        TaskAction::MarkDone => "mark_done",
        TaskAction::Retry => "retry",
        TaskAction::ToTop => "to_top",
    }
}

/// Cosa chiede il segretario.
#[derive(Clone, Copy)]
pub struct RequestParams<'a> {
    pub task_id: &'a str,
    pub action: TaskAction,
    /// Gia' ripulito e nei limiti di `REASON_MAX`.
    pub reason: &'a str,
    pub secretary: &'a str,
}

/// La nuova richiesta con id `id` (quello del messaggio della scheda), in
/// attesa fino a `now + REQUEST_TTL_MS`; o il motivo del rifiuto, in inglese.
pub fn open_request(
    thread: &Thread,
    view: &BoardView,
    params: RequestParams<'_>,
    id: String,
    now: i64,
) -> Result<HubActionRequest, String> {
    let RequestParams { task_id, action, reason, secretary } = params;
    if !REQUESTABLE.contains(&action) {
        return Err(format!("`{}` cannot be requested.", action_code(action)));
    }
    if thread.channel != Channel::Workspace(view.workspace_id.clone()) {
        return Err(NOT_A_WORKSPACE_THREAD.to_string());
    }
    let shown_id = clean_line(task_id, TITLE_KEPT);
    if !thread.task_ids.iter().any(|t| t == task_id) {
        return Err(format!(
            "Task \"{shown_id}\" is not linked to this thread: you can request actions only on the tasks of this thread."
        ));
    }
    let task =
        view.tasks.iter().find(|t| t.task.id == task_id).ok_or_else(|| {
            format!("No task with id \"{shown_id}\" on the board: use `board_status` to find task ids.")
        })?;
    require_possible(task, action)?;
    if open_request_of(thread, task_id, now).is_some() {
        return Err(format!(
            "There is already an open request for task \"{shown_id}\", waiting for the user. Wait for its outcome \
            before asking again."
        ));
    }
    Ok(HubActionRequest {
        id,
        action,
        reason: reason.to_string(),
        secretary: secretary.to_string(),
        task_title: clean_line(&task.task.title, TITLE_KEPT),
        task_status: task.task.status().as_str().to_string(),
        dependents: dependents_of(view, task, action),
        expires_at: now + REQUEST_TTL_MS,
        state: ActionRequestState::Pending,
        resolved_at: None,
        note: None,
    })
}

/// Apre la richiesta e ne scrive la scheda nel thread `thread_id` (autore di
/// sistema: il corpo dice chi ha chiesto). Ritorna l'esito per il modello e il
/// thread aggiornato. Il chiamante la esegue sotto il lock dei thread.
pub fn post_request(
    list: &mut ChannelThreads,
    thread_id: &str,
    view: &BoardView,
    params: RequestParams<'_>,
    next_id: &mut dyn FnMut(&'static str) -> String,
    now: i64,
) -> Result<(Value, ThreadSummary), String> {
    let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    let id = next_id(MESSAGE_ID_PREFIX);
    let request = open_request(thread, view, params, id.clone(), now)?;
    let card = NewMessage {
        id,
        author: Author::System,
        body: texts::request_card(&request),
        reply_to: None,
        event: Some(HubEvent {
            workspace: Some(view.workspace_id.clone()),
            task_id: Some(params.task_id.to_string()),
            status: Some(request.task_status.clone()),
            request: Some(request.clone()),
            ..HubEvent::new(HubEventKind::ActionRequest)
        }),
        origin: None,
    };
    list.add_message(thread_id, card, now).map_err(|e| {
        log::warn!("hub: scheda della richiesta non scritta nel thread {thread_id}: {e}");
        CARD_NOT_POSTED.to_string()
    })?;
    let touched = list.thread(thread_id).map(summary).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok((requested(&request, params.task_id), touched))
}

/// Il task permette l'azione adesso (e una cascata non tocca task in corso).
fn require_possible(task: &TaskView, action: TaskAction) -> Result<(), String> {
    if !task.allowed_actions.contains(&action) {
        return Err(format!("{} and `{}` is not allowed on it now.", describe(task), action_code(action)));
    }
    if action == TaskAction::DeleteCascade && !task.cascade.running.is_empty() {
        return Err(format!(
            "{} and `delete_cascade` would remove running tasks ({}): stop them or wait for them first.",
            describe(task),
            task.cascade.running.join(", ")
        ));
    }
    Ok(())
}

/// I titoli dei task che una cascata toglierebbe insieme a `task`.
fn dependents_of(view: &BoardView, task: &TaskView, action: TaskAction) -> Vec<String> {
    if action != TaskAction::DeleteCascade {
        return Vec::new();
    }
    let title = |id: &String| {
        let found = view.tasks.iter().find(|t| &t.task.id == id);
        clean_line(found.map_or(id.as_str(), |t| t.task.title.as_str()), TITLE_KEPT)
    };
    task.cascade.dependents.iter().map(title).collect()
}

/// La richiesta ancora aperta del task: in attesa e non scaduta.
fn open_request_of<'a>(thread: &'a Thread, task_id: &str, now: i64) -> Option<&'a HubActionRequest> {
    requests(&thread.messages)
        .filter(|(event_task, _)| *event_task == Some(task_id))
        .map(|(_, request)| request)
        .find(|r| r.state == ActionRequestState::Pending && now < r.expires_at)
}

/// Le schede di richiesta del thread, con il task a cui si riferiscono.
fn requests(messages: &[Message]) -> impl Iterator<Item = (Option<&str>, &HubActionRequest)> {
    messages
        .iter()
        .filter_map(|m| m.event.as_ref())
        .filter(|e| e.kind == HubEventKind::ActionRequest)
        .filter_map(|e| e.request.as_ref().map(|r| (e.task_id.as_deref(), r)))
}

/// L'esito per il modello di una richiesta appena aperta.
pub fn requested(request: &HubActionRequest, task_id: &str) -> Value {
    json!({
        "request_id": request.id,
        "task_id": task_id,
        "title": request.task_title,
        "action": action_code(request.action),
        "status": "awaiting_confirmation",
        "expires_in_minutes": REQUEST_TTL_MS / MS_PER_MINUTE,
        "note": "Nothing was done yet: the request is a card in the thread and only the user can confirm it with a \
            click. You will be woken up with the outcome. Tell the user that you asked, and do not say the action \
            was done.",
    })
}

/// Cosa fare di una richiesta dopo il clic dell'utente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Confermata e ancora valida: eseguire l'azione.
    Execute(TaskAction),
    /// L'utente ha annullato.
    Cancel,
    /// Scaduta prima della conferma: nessuna azione.
    Expired,
    /// Il task non e' piu' com'era: nessuna azione, con la spiegazione (italiano).
    Stale(&'static str),
    /// Gia' decisa o decaduta: non si decide due volte.
    AlreadyResolved,
}

/// Decide cosa fare al clic: `task` e' il task com'e' adesso sulla board
/// (`None` se non c'e' piu'). L'annullo vale anche su una richiesta scaduta
/// ma non ancora decisa; la conferma solo prima della scadenza (`now <
/// expires_at`) e se il task ha ancora lo stato di quando e' stata chiesta.
pub fn decide(request: &HubActionRequest, task: Option<&TaskView>, now: i64, confirm: bool) -> Resolution {
    if request.state != ActionRequestState::Pending {
        return Resolution::AlreadyResolved;
    }
    if !confirm {
        return Resolution::Cancel;
    }
    if now >= request.expires_at {
        return Resolution::Expired;
    }
    let Some(task) = task else {
        return Resolution::Stale(texts::TASK_GONE);
    };
    if task.task.status().as_str() != request.task_status {
        return Resolution::Stale(texts::TASK_CHANGED);
    }
    let cascade_blocked = request.action == TaskAction::DeleteCascade && !task.cascade.running.is_empty();
    if !task.allowed_actions.contains(&request.action) || cascade_blocked {
        return Resolution::Stale(texts::ACTION_NOT_ALLOWED);
    }
    Resolution::Execute(request.action)
}

/// Il task `task_id` del thread `thread_id` ha cambiato stato (`status`) o e'
/// sparito (`None`): le sue richieste in attesa che erano per un altro stato
/// diventano `stale`. Mai quelle gia' decise. Ritorna quante ne ha invalidate.
pub fn invalidate_for_task(
    list: &mut ChannelThreads,
    thread_id: &str,
    task_id: &str,
    status: Option<&str>,
    now: i64,
) -> Result<usize, String> {
    let messages = list.thread(thread_id).map_or(&[][..], |t| &t.messages[..]);
    let outdated: Vec<String> = messages
        .iter()
        .filter(|m| {
            let Some(event) = m.event.as_ref().filter(|e| e.task_id.as_deref() == Some(task_id)) else {
                return false;
            };
            let pending_for_other_status =
                |r: &HubActionRequest| r.state == ActionRequestState::Pending && status != Some(r.task_status.as_str());
            event.kind == HubEventKind::ActionRequest && event.request.as_ref().is_some_and(pending_for_other_status)
        })
        .map(|m| m.id.clone())
        .collect();
    let note = if status.is_some() { texts::TASK_CHANGED } else { texts::TASK_GONE };
    for message_id in &outdated {
        list.update_request(thread_id, message_id, now, |request| {
            request.state = ActionRequestState::Stale;
            request.resolved_at = Some(now);
            request.note = Some(note.to_string());
        })?;
    }
    Ok(outdated.len())
}

#[cfg(test)]
mod tests;
