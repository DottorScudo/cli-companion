//! `relay_answer` dei segretari (puro, generico sulla board): l'utente chiede al
//! segretario di rispondere al posto suo a una domanda di un worker ("@mario
//! digli di usare la seconda opzione"). Il tool non prende un id: il motore, a
//! inizio turno, lo lega all'**unica** domanda in attesa del thread
//! (`relay_target`), e solo in un turno svegliato da un messaggio dell'utente;
//! un risveglio da evento della board non puo' mai rispondere. Con piu'
//! domande in attesa il segretario deve chiedere quale e l'utente risponde con
//! "Rispondi" sull'evento (risposta diretta, senza modello).
//!
//! Gli errori sono in inglese: tornano al modello.

use super::sessions::WakeCause;
use crate::hub::model::Thread;
use crate::hub::questions::{pending_questions, QuestionBoard, QuestionRef};
use serde_json::{json, Value};

const NOT_FROM_USER: &str = "relay_answer works only when the user's message woke you up and asks you to answer \
a worker's question on their behalf, never when a board event woke you up. Do not answer: present the question \
and tell the user to use Reply on the question message, or to ask you to answer it.";
const NO_QUESTION: &str = "No worker question is waiting for an answer in this thread. Tell the user.";
const SEVERAL_QUESTIONS: &str = "More than one worker question is waiting for an answer in this thread, so you \
cannot tell which one to answer. Ask the user which question they mean and tell them to use Reply on that \
question message: their reply is delivered to the worker as written.";

/// La domanda a cui il turno puo' rispondere: l'unica in attesa nel thread, in
/// un turno svegliato da un messaggio dell'utente.
pub fn relay_target(thread: &Thread, cause: WakeCause) -> Result<QuestionRef, &'static str> {
    if cause != WakeCause::Message {
        return Err(NOT_FROM_USER);
    }
    let mut pending = pending_questions(thread);
    match pending.len() {
        0 => Err(NO_QUESTION),
        1 => Ok(pending.remove(0)),
        _ => Err(SEVERAL_QUESTIONS),
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
