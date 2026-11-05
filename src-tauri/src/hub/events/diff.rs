//! Il diff puro tra due istantanee della stessa board: la lista degli eventi
//! da riportare nei thread, nell'ordine in cui instradarli (prima le proposte,
//! poi i task nell'ordine della board, infine i task spariti).

use super::snapshot::{BoardSnapshot, ProposalSnap, TaskSnap};
use crate::board::TaskStatus;
use crate::hub::model::HubQuestion;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardEvent {
    ProposalCreated(ProposalSnap),
    /// La proposta e' sparita e i suoi task (`task_ids`, quelli comparsi) sono
    /// ora nella board.
    ProposalAccepted {
        proposal_id: String,
        task_ids: Vec<String>,
    },
    /// La proposta e' sparita senza portare task nella board.
    ProposalRejected {
        proposal_id: String,
    },
    /// Un cambiamento di un task; `task` e' il task com'e' adesso (per un task
    /// eliminato, com'era l'ultima volta).
    Task {
        change: TaskChange,
        task: TaskSnap,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskChange {
    /// Comparso senza venire da una proposta nota: creato a mano.
    Created,
    Started,
    Done,
    Failed,
    /// Il worker si e' messo ad aspettare l'utente.
    Attention,
    /// Il worker ha fatto una domanda all'utente: `task.question` e' la domanda.
    Question,
    /// La domanda in attesa non aspetta piu' e il task e' ancora in corso (ha
    /// avuto una risposta): `task.question` e' la domanda appena risposta, non
    /// quella di adesso.
    QuestionAnswered,
    /// Sparito senza essere `done` (uno sparito da `done` e' stato archiviato).
    Deleted,
}

pub fn diff(prev: &BoardSnapshot, next: &BoardSnapshot) -> Vec<BoardEvent> {
    let prev_tasks: HashMap<&str, &TaskSnap> = prev.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let next_ids: HashSet<&str> = next.tasks.iter().map(|t| t.id.as_str()).collect();
    let appeared = |id: &str| next_ids.contains(id) && !prev_tasks.contains_key(id);

    let mut events = Vec::new();
    let mut from_proposals: HashSet<&str> = HashSet::new();
    let prev_proposals: HashSet<&str> = prev.proposals.iter().map(|p| p.id.as_str()).collect();
    let next_proposals: HashSet<&str> = next.proposals.iter().map(|p| p.id.as_str()).collect();
    for p in next.proposals.iter().filter(|p| !prev_proposals.contains(p.id.as_str())) {
        events.push(BoardEvent::ProposalCreated(p.clone()));
    }
    for p in prev.proposals.iter().filter(|p| !next_proposals.contains(p.id.as_str())) {
        let task_ids: Vec<String> = p.tasks.iter().filter(|t| appeared(&t.id)).map(|t| t.id.clone()).collect();
        if task_ids.is_empty() {
            events.push(BoardEvent::ProposalRejected { proposal_id: p.id.clone() });
        } else {
            from_proposals.extend(p.tasks.iter().map(|t| t.id.as_str()));
            events.push(BoardEvent::ProposalAccepted { proposal_id: p.id.clone(), task_ids });
        }
    }

    for task in &next.tasks {
        let before = prev_tasks.get(task.id.as_str()).copied();
        if before.is_none() && !from_proposals.contains(task.id.as_str()) {
            events.push(task_event(TaskChange::Created, task));
        }
        // Un task appena comparso parte da "in coda, senza attenzione".
        let (status, attention) = before.map_or((TaskStatus::Queued, false), |b| (b.status, b.attention));
        if let Some(change) = status_change(status, task.status) {
            events.push(task_event(change, task));
        }
        if task.attention && !attention {
            events.push(task_event(TaskChange::Attention, task));
        }
        question_events(before.and_then(|b| b.question.as_ref()), task, &mut events);
    }

    for gone in prev.tasks.iter().filter(|t| !next_ids.contains(t.id.as_str())) {
        if gone.status != TaskStatus::Done {
            events.push(task_event(TaskChange::Deleted, gone));
        }
    }
    events
}

/// L'evento di un cambio di stato: `queued` non ne ha (un "Riprova" o uno
/// stop che rimette in coda si vedra' alla ripartenza).
fn status_change(before: TaskStatus, after: TaskStatus) -> Option<TaskChange> {
    if before == after {
        return None;
    }
    match after {
        TaskStatus::Queued => None,
        TaskStatus::Running => Some(TaskChange::Started),
        TaskStatus::Done => Some(TaskChange::Done),
        TaskStatus::Failed => Some(TaskChange::Failed),
    }
}

/// Gli eventi della domanda in attesa del task: prima la risposta a quella di
/// prima (solo con il task ancora in corso: un task concluso ha gia' il suo
/// evento), poi la domanda nuova.
fn question_events(before: Option<&HubQuestion>, task: &TaskSnap, events: &mut Vec<BoardEvent>) {
    let now = task.question.as_ref();
    if before.map(|q| &q.id) == now.map(|q| &q.id) {
        return;
    }
    if let Some(answered) = before.filter(|_| task.status == TaskStatus::Running) {
        let task = TaskSnap { question: Some(answered.clone()), ..task.clone() };
        events.push(BoardEvent::Task { change: TaskChange::QuestionAnswered, task });
    }
    if now.is_some() {
        events.push(task_event(TaskChange::Question, task));
    }
}

fn task_event(change: TaskChange, task: &TaskSnap) -> BoardEvent {
    BoardEvent::Task { change, task: task.clone() }
}

#[cfg(test)]
mod tests;
