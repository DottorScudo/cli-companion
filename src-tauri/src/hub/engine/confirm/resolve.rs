//! Il clic dell'utente su una richiesta: decide (`decide`), esegue l'azione
//! sulla board solo se la conferma vale, aggiorna la scheda e scrive la riga
//! dell'esito in risposta alla scheda. Il chiamante la esegue sotto il lock
//! dei thread: due clic sulla stessa scheda non possono eseguirla due volte.

use super::{decide, texts, Resolution};
use crate::board::{TaskAction, TaskView};
use crate::hub::channel::Channel;
use crate::hub::model::{
    ActionRequestState, Author, HubActionRequest, HubEvent, HubEventKind, RequestedAction, ThreadSummary,
};
use crate::hub::state::MESSAGE_ID_PREFIX;
use crate::hub::text::clean_line;
use crate::hub::threads::{summary, ChannelThreads, NewMessage, THREAD_NOT_FOUND};
use serde_json::Value;

/// Caratteri tenuti dell'errore della board nella nota della richiesta.
const BOARD_ERROR_KEPT: usize = 300;
const REQUEST_NOT_FOUND: &str = "richiesta non trovata";
const NOT_A_BOARD_THREAD: &str = "la richiesta non appartiene a un workspace";

/// La board vista da chi decide una richiesta.
pub trait ActionBoard {
    /// Il task com'e' adesso sulla board del workspace, se c'e'.
    fn task(&self, workspace: &str, task_id: &str) -> Option<TaskView>;
    /// Esegue l'azione, con le stesse regole dei comandi del pannello Task.
    /// Gli errori sono per l'utente (italiano).
    fn act(&self, workspace: &str, task_id: &str, action: TaskAction) -> Result<(), String>;
    /// Cambia i campi del task con `patch`, con la validazione di `update_task`
    /// e le stesse regole del pannello Task. Gli errori sono per l'utente.
    fn update(&self, workspace: &str, task_id: &str, patch: &Value) -> Result<(), String>;
}

/// Esegue sulla board cio' che la richiesta chiede: l'azione, o la modifica
/// con i valori completi salvati nella richiesta.
fn perform(
    board: &impl ActionBoard,
    workspace: &str,
    task_id: &str,
    request: &HubActionRequest,
    action: RequestedAction,
) -> Result<(), String> {
    match (action.task_action(), &request.update) {
        (Some(task_action), _) => board.act(workspace, task_id, task_action),
        (None, Some(update)) => board.update(workspace, task_id, &update.patch),
        (None, None) => Err(REQUEST_NOT_FOUND.to_string()),
    }
}

/// L'esito di un clic.
#[derive(Debug)]
pub struct Resolved {
    /// La richiesta com'e' ora.
    pub request: HubActionRequest,
    /// La riga dell'esito: il messaggio che sveglia chi ha fatto la richiesta.
    pub outcome_id: String,
    /// Il thread dopo la scrittura.
    pub touched: ThreadSummary,
}

/// Applica il clic (`confirm`: conferma o annulla) alla richiesta del messaggio
/// `message_id`. Una richiesta gia' decisa e' un errore e non cambia nulla;
/// tutti gli altri esiti (scaduta, non piu' valida, rifiutata dalla board)
/// aggiornano la scheda e scrivono la riga dell'esito. `next_id` da' gli id dei messaggi.
pub fn resolve(
    list: &mut ChannelThreads,
    thread_id: &str,
    message_id: &str,
    confirm: bool,
    now: i64,
    next_id: &mut dyn FnMut(&'static str) -> String,
    board: &impl ActionBoard,
) -> Result<Resolved, String> {
    let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    let Channel::Workspace(workspace) = &thread.channel else {
        return Err(NOT_A_BOARD_THREAD.to_string());
    };
    let workspace = workspace.clone();
    let event = thread
        .messages
        .iter()
        .find(|m| m.id == message_id)
        .and_then(|m| m.event.clone())
        .filter(|e| e.kind == HubEventKind::ActionRequest)
        .ok_or_else(|| REQUEST_NOT_FOUND.to_string())?;
    let request = event.request.clone().ok_or_else(|| REQUEST_NOT_FOUND.to_string())?;
    let task_id = event.task_id.clone().ok_or_else(|| REQUEST_NOT_FOUND.to_string())?;
    let task = board.task(&workspace, &task_id);
    let (state, note) = match decide(&request, task.as_ref(), now, confirm) {
        Resolution::AlreadyResolved => return Err(texts::ALREADY_DECIDED.to_string()),
        Resolution::Cancel => (ActionRequestState::Cancelled, None),
        Resolution::Expired => (ActionRequestState::Expired, Some(texts::EXPIRED_NOTE.to_string())),
        Resolution::Stale(reason) => (ActionRequestState::Stale, Some(reason.to_string())),
        Resolution::Execute(action) => match perform(board, &workspace, &task_id, &request, action) {
            Ok(()) => (ActionRequestState::Confirmed, None),
            Err(e) => {
                log::warn!("hub: azione {action:?} confermata sul task {task_id} rifiutata dalla board: {e}");
                (ActionRequestState::Failed, Some(clean_line(&e, BOARD_ERROR_KEPT)))
            }
        },
    };
    let request = list.update_request(thread_id, message_id, now, |request| {
        request.state = state;
        request.resolved_at = Some(now);
        request.note = note;
        request.clone()
    })?;
    let outcome = NewMessage {
        id: next_id(MESSAGE_ID_PREFIX),
        author: Author::System,
        body: texts::outcome(&request),
        reply_to: Some(message_id.to_string()),
        event: Some(HubEvent {
            workspace: Some(workspace),
            task_id: Some(task_id),
            status: Some(request.task_status.clone()),
            request: Some(request.clone()),
            ..HubEvent::new(HubEventKind::ActionRequestResolved)
        }),
        origin: None,
    };
    let outcome_id = list.add_message(thread_id, outcome, now)?.id.clone();
    let touched = list.thread(thread_id).map(summary).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok(Resolved { request, outcome_id, touched })
}

#[cfg(test)]
mod tests;
