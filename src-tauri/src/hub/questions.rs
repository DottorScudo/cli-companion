//! Le domande dei worker in un thread (puro): quali aspettano ancora una
//! risposta e a quale domanda risponde un messaggio. Le usano la risposta
//! diretta (`direct_answer`, "Rispondi" sull'evento) e il tool `relay_answer`
//! dei segretari. Lo stato vero della domanda e' sulla board: qui si legge solo
//! cio' che il thread ha riportato, e la board ricontrolla alla consegna.

use super::channel::Channel;
use super::model::{HubEvent, HubEventKind, Subject, Thread};
use std::collections::HashMap;

/// Una domanda a cui si puo' rispondere: il workspace e il task della board e
/// l'id della domanda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionRef {
    pub workspace: String,
    pub task_id: String,
    pub question_id: String,
}

/// La domanda citata non aspetta piu' una risposta nel thread: ha gia' avuto
/// una risposta, o il suo task e' andato avanti (concluso, fallito, ripartito).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotPending;

/// La board vista da chi consegna una risposta.
pub trait QuestionBoard {
    /// Consegna `text` cosi' com'e' alla domanda: la board la segna risposta e la
    /// scrive nel terminale del worker. Gli errori sono per l'utente (italiano).
    fn answer(&self, question: &QuestionRef, text: &str) -> Result<(), String>;
}

/// Le domande che aspettano una risposta nel thread, dalla piu' vecchia: per
/// ogni task conta il suo ultimo evento, che deve essere una domanda del
/// workspace del thread.
pub fn pending_questions(thread: &Thread) -> Vec<QuestionRef> {
    let mut last: HashMap<&str, (usize, &HubEvent)> = HashMap::new();
    for (at, event) in thread.messages.iter().enumerate().filter_map(|(at, m)| m.event.as_ref().map(|e| (at, e))) {
        if let Some(Subject::Task(task_id)) = event.subject() {
            last.insert(task_id, (at, event));
        }
    }
    let mut pending: Vec<(usize, QuestionRef)> =
        last.into_values().filter_map(|(at, event)| question_of(thread, event).map(|q| (at, q))).collect();
    pending.sort_by_key(|(at, _)| *at);
    pending.into_iter().map(|(_, q)| q).collect()
}

/// La domanda a cui risponde un messaggio con `reply_to` = `target_id`: `None`
/// se il messaggio citato non e' una domanda (un messaggio qualunque),
/// `Some(Err)` se lo e' ma non aspetta piu' una risposta.
pub fn replied_question(thread: &Thread, target_id: &str) -> Option<Result<QuestionRef, NotPending>> {
    let event = thread.messages.iter().find(|m| m.id == target_id)?.event.as_ref()?;
    if event.kind != HubEventKind::TaskQuestion {
        return None;
    }
    let pending = question_of(thread, event).filter(|q| pending_questions(thread).contains(q));
    Some(pending.ok_or(NotPending))
}

/// La domanda di un evento `task_question` della board del thread.
fn question_of(thread: &Thread, event: &HubEvent) -> Option<QuestionRef> {
    if event.kind != HubEventKind::TaskQuestion {
        return None;
    }
    let workspace = event.workspace.as_deref()?;
    if thread.channel != Channel::Workspace(workspace.to_string()) {
        return None;
    }
    Some(QuestionRef {
        workspace: workspace.to_string(),
        task_id: event.task_id.clone()?,
        question_id: event.question.as_ref()?.id.clone(),
    })
}

#[cfg(test)]
pub mod fixtures;
#[cfg(test)]
mod tests;
