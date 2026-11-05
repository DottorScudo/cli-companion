//! Le domande dei worker all'utente (`ask_user`): il tool, la chiusura della
//! domanda quando l'utente risponde nel terminale e la consegna della risposta
//! data dall'app (l'Agents HUB la chiama con `answer_question`). Lo stato della
//! domanda sta sul task (`board::TaskQuestion`) e lo scrive solo `lifecycle`; qui
//! si decide quando e si scrive nel terminale, mai sotto il lock.

use super::*;
use crate::board::{parse_question, ANSWER_MAX};
use serde_json::json;

/// `ask_user` senza l'Agents HUB (lo legge il modello).
pub(super) const HUB_OFF: &str =
    "ask_user is not available: the Agents HUB is off. Ask in plain text and end your turn.";

/// Una seconda domanda mentre la prima aspetta (lo legge il modello).
const QUESTION_PENDING: &str =
    "a question is already waiting for the user's answer: end your turn now, the answer will arrive in this terminal";

/// Risposta di `ask_user` (la legge il modello).
pub const ASK_USER_SENT: &str =
    "Question sent to the user. End your turn now: the answer will arrive as the user's next message in this terminal.";

/// Il worker della domanda non c'e' piu' (per l'interfaccia).
const WORKER_GONE: &str =
    "Il worker del task non è più attivo (tab chiuso o lavoro finito): la domanda resta registrata.";

/// Pausa fra il testo della risposta e l'invio: i TUI possono trattare un ritorno
/// carrello arrivato nella stessa lettura del testo come parte di un incolla (un
/// a capo) e non come un invio.
const SUBMIT_DELAY: Duration = Duration::from_millis(150);

/// `ask_user`: registra la domanda del worker sul suo task. Non notifica: l'utente
/// viene avvisato quando il worker finisce il turno e si mette ad aspettare.
pub(super) fn ask_user<H: Host>(
    st: &AutomationState,
    host: &H,
    session: &str,
    params: &Value,
) -> Result<Value, String> {
    let (text, options) = parse_question(params)?;
    st.run(host, |r| r.ask_user(session, text, options, now_millis()))
}

impl Registry {
    fn ask_user(&mut self, session: &str, text: String, options: Vec<String>, now: i64) -> Result<Value, String> {
        let w = self.live_worker(session)?;
        let (ws, tid) = (w.workspace_id.clone(), w.task_id.clone());
        let unassigned = || TASK_UNASSIGNED.to_string();
        let board = self.boards.get_mut(&ws).ok_or_else(unassigned)?;
        let task = board.task(&tid).filter(|t| t.is_live_for(session)).ok_or_else(unassigned)?;
        if task.pending_question().is_some() {
            return Err(QUESTION_PENDING.to_string());
        }
        let id = board.ask_question(&tid, session, text, options, now).map_err(|_| unassigned())?;
        self.mark_dirty(&ws);
        self.touch(session, now);
        Ok(json!({ "question_id": id, "message": ASK_USER_SENT }))
    }

    /// Il task del worker `session` ha una domanda in attesa di risposta.
    pub(super) fn has_pending_question(&self, session: &str) -> bool {
        let Some(w) = self.workers.get(session) else {
            return false;
        };
        let task = self.boards.get(&w.workspace_id).and_then(|b| b.task(&w.task_id));
        task.is_some_and(|t| t.is_live_for(session) && t.pending_question().is_some())
    }

    /// Chiude la domanda `question_id` del task e rimette il worker al lavoro.
    /// `answer` = testo consegnato dall'app, `None` = risposta data nel terminale.
    fn close_question(&mut self, session: &str, question_id: &str, answer: Option<String>, now: i64) -> bool {
        let Some(w) = self.workers.get(session) else {
            return false;
        };
        let (ws, tid) = (w.workspace_id.clone(), w.task_id.clone());
        let closed = self
            .boards
            .get_mut(&ws)
            .is_some_and(|b| b.answer_question(&tid, session, question_id, answer, now).is_ok());
        if closed {
            self.mark_dirty(&ws);
            self.touch(session, now);
        }
        closed
    }

    /// L'utente ha risposto direttamente nel terminale del worker in attesa: la
    /// domanda si chiude senza testo. Un permesso in attesa (`needs_input`) non
    /// c'entra: li' l'invio approva il permesso.
    pub(super) fn close_question_answered_in_terminal(&mut self, session: &str, now: i64) {
        let Some(w) = self.workers.get(session) else {
            return;
        };
        let task = self.boards.get(&w.workspace_id).and_then(|b| b.task(&w.task_id));
        if let Some(id) = task.and_then(|t| t.pending_question()).map(|q| q.id.clone()) {
            self.close_question(session, &id, None, now);
        }
    }

    /// Controlla che la domanda si possa rispondere adesso e la segna risposta con
    /// `clean`; ritorna il tab del worker, a cui l'app deve ancora scrivere. Gli
    /// errori sono per l'interfaccia (italiano).
    pub(super) fn begin_answer(
        &mut self,
        ws_id: &str,
        task_id: &str,
        question_id: &str,
        clean: &str,
        now: i64,
    ) -> Result<String, String> {
        let task = self.boards.get(ws_id).and_then(|b| b.task(task_id)).ok_or("Task non trovato.")?;
        let question = task.questions().iter().find(|q| q.id == question_id).ok_or("Domanda non trovata.")?;
        if question.answered_at.is_some() {
            return Err("La domanda ha già una risposta.".to_string());
        }
        let pending = task.pending_question().is_some_and(|q| q.id == question_id);
        let session = task.session().filter(|_| pending).map(str::to_string).ok_or(WORKER_GONE)?;
        if !self.workers.get(&session).is_some_and(|w| w.tab_open) {
            return Err(WORKER_GONE.to_string());
        }
        match self.worker_activity(&session).ok_or(WORKER_GONE)? {
            WorkerActivity::AwaitingAnswer | WorkerActivity::Idle => {}
            WorkerActivity::NeedsInput => {
                return Err("Il worker aspetta un permesso nel terminale: rispondi lì prima di inviare la risposta."
                    .to_string());
            }
            WorkerActivity::Starting | WorkerActivity::Running | WorkerActivity::Background => {
                return Err(
                    "Il worker sta ancora lavorando: la risposta si può inviare quando si ferma in attesa.".to_string()
                );
            }
        }
        if !self.close_question(&session, question_id, Some(clean.to_string()), now) {
            return Err(WORKER_GONE.to_string());
        }
        Ok(session)
    }

    /// La risposta non e' arrivata nel terminale: la domanda torna in attesa.
    fn answer_undelivered(&mut self, ws_id: &str, task_id: &str, session: &str, question_id: &str, now: i64) {
        let undone =
            self.boards.get_mut(ws_id).is_some_and(|b| b.answer_undelivered(task_id, session, question_id, now));
        if undone {
            self.mark_dirty(ws_id);
            self.touch(session, now);
        }
    }
}

impl AutomationState {
    /// Consegna la risposta dell'utente a una domanda del worker: la segna sul task
    /// e la digita nel terminale del worker (`answer_input`). Se il terminale non e'
    /// raggiungibile la domanda torna in attesa. Nessun lock e' in mano mentre si scrive.
    pub fn answer_question<H: Host>(
        &self,
        host: &H,
        ws_id: &str,
        task_id: &str,
        question_id: &str,
        text: &str,
    ) -> Result<(), String> {
        let clean = answer_input::sanitize(text);
        if clean.is_empty() {
            return Err("La risposta è vuota.".to_string());
        }
        if clean.chars().count() > ANSWER_MAX {
            return Err(format!("La risposta è troppo lunga (massimo {ANSWER_MAX} caratteri)."));
        }
        let session = self.run(host, |r| r.begin_answer(ws_id, task_id, question_id, &clean, now_millis()))?;
        let keys = answer_input::keystrokes(&clean);
        if host.write_to_tab(&session, &keys.text).is_err() {
            self.run(host, |r| r.answer_undelivered(ws_id, task_id, &session, question_id, now_millis()));
            return Err("Il terminale del worker non è raggiungibile: la domanda resta in attesa.".to_string());
        }
        let writer = host.clone();
        host.defer(
            SUBMIT_DELAY,
            Box::new(move || {
                if let Err(e) = writer.write_to_tab(&session, &keys.submit) {
                    log::error!("answer_question: invio non riuscito nel tab {session}: {e}");
                }
            }),
        );
        Ok(())
    }
}
