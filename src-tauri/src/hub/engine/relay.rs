//! `relay_answer` dei segretari (puro, generico sulla board): l'utente chiede al
//! segretario di rispondere al posto suo a una domanda di un worker ("@mario
//! digli di usare la seconda opzione"). A inizio turno il motore ricava le
//! domande in attesa del thread (`relay_candidates`), e solo in un turno
//! svegliato da un messaggio dell'utente: un risveglio da evento della board o
//! da un elemento esterno non puo' mai rispondere. Il tool sceglie fra quelle
//! (`choose`): senza id vale l'unica domanda in attesa; con piu' domande il
//! segretario passa `question_id` (e `task_id`, perche' l'id di una domanda e'
//! unico solo nel suo task) letti da `thread_read`. Un id estraneo al thread e'
//! rifiutato; il segretario non risponde mai di sua iniziativa.
//!
//! Gli errori sono in inglese: tornano al modello.

use super::sessions::WakeCause;
use crate::hub::model::Thread;
use crate::hub::questions::{pending_questions, QuestionBoard, QuestionRef};
use serde_json::{json, Value};

const NOT_FROM_USER: &str = "relay_answer works only when the user's message woke you up and asks you to answer \
a worker's question on their behalf, never when a board event or external content woke you up. Do not answer: present the question \
and tell the user to use Reply on the question message, or to ask you to answer it.";
const NO_QUESTION: &str = "No worker question is waiting for an answer in this thread. Tell the user.";
const SEVERAL_QUESTIONS: &str = "More than one worker question is waiting for an answer in this thread, so you \
cannot tell which one to answer. If the user said which one, call again with its `question_id` and `task_id` (read \
them with `thread_read`); otherwise ask the user which question they mean and tell them to use Reply on that \
question message: their reply is delivered to the worker as written.";
const UNKNOWN_QUESTION: &str = "No question waiting for an answer in this thread matches that `question_id` and \
`task_id`. Read the thread with `thread_read` to see the waiting questions, or tell the user to use Reply on the \
question message.";
const AMBIGUOUS_QUESTION: &str = "Several waiting questions have that `question_id`, because it is unique only within \
its task. Call again with the `task_id` of the question the user means.";

/// Le domande a cui il turno puo' rispondere, o perche' non puo' rispondere.
pub type RelayCandidates = Result<Vec<QuestionRef>, &'static str>;

/// Quale domanda in attesa vuole il modello; tutto vuoto = l'unica in attesa.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestionSelector {
    pub question_id: Option<String>,
    pub task_id: Option<String>,
}

impl QuestionSelector {
    fn is_empty(&self) -> bool {
        self.question_id.is_none() && self.task_id.is_none()
    }

    fn matches(&self, question: &QuestionRef) -> bool {
        self.question_id.as_ref().is_none_or(|id| *id == question.question_id)
            && self.task_id.as_ref().is_none_or(|id| *id == question.task_id)
    }
}

/// Le domande in attesa del thread, in un turno svegliato da un messaggio
/// dell'utente.
pub fn relay_candidates(thread: &Thread, cause: WakeCause) -> RelayCandidates {
    if cause != WakeCause::Message {
        return Err(NOT_FROM_USER);
    }
    match pending_questions(thread) {
        pending if pending.is_empty() => Err(NO_QUESTION),
        pending => Ok(pending),
    }
}

/// La domanda scelta fra quelle in attesa: solo una di esse, mai un id estraneo.
pub fn choose(candidates: &RelayCandidates, selector: &QuestionSelector) -> Result<QuestionRef, &'static str> {
    let pending = candidates.as_ref().map_err(|e| *e)?;
    let mut matching = pending.iter().filter(|q| selector.matches(q));
    match (matching.next(), matching.next()) {
        (Some(only), None) => Ok(only.clone()),
        (None, _) => Err(UNKNOWN_QUESTION),
        (Some(_), Some(_)) if selector.is_empty() => Err(SEVERAL_QUESTIONS),
        (Some(_), Some(_)) => Err(AMBIGUOUS_QUESTION),
    }
}

/// `relay_answer`: consegna `body` alla domanda del turno.
pub fn relay(board: &impl QuestionBoard, target: Result<&QuestionRef, &str>, body: &str) -> Result<Value, String> {
    let target = target.map_err(str::to_string)?;
    board.answer(target, body).map_err(|e| {
        log::warn!("hub: risposta alla domanda {} non consegnata dal segretario: {e}", target.question_id);
        format!("The answer could not be delivered to the worker: {e}")
    })?;
    Ok(json!({ "task_id": target.task_id, "question_id": target.question_id, "delivered": true }))
}

#[cfg(test)]
mod tests;
