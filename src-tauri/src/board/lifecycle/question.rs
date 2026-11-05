//! Le domande del worker all'utente: l'unico codice che scrive `Task::questions`
//! e l'attivita' `AwaitingAnswer` che le accompagna.

use super::LifecycleError;
use crate::board::{Run, RunPhase, Task, TaskQuestion, TaskState, WorkerActivity, QUESTIONS_KEPT};

impl Task {
    /// L'attivita' del worker vivo `session`, modificabile.
    fn live_worker_mut(&mut self, session: &str) -> Result<&mut WorkerActivity, LifecycleError> {
        match &mut self.state {
            TaskState::Running(Run { phase: RunPhase::Live { session: s, worker }, .. }) if s == session => Ok(worker),
            _ => Err(LifecycleError::Stale),
        }
    }

    /// La domanda in attesa, se e' proprio `id`.
    fn pending_with_id(&mut self, id: &str) -> Result<&mut TaskQuestion, LifecycleError> {
        if self.pending_question().is_none_or(|q| q.id != id) {
            return Err(LifecycleError::Stale);
        }
        self.questions.last_mut().ok_or(LifecycleError::Stale)
    }

    pub(super) fn question_asked(
        &mut self,
        session: &str,
        text: String,
        options: Vec<String>,
        now: i64,
    ) -> Result<(), LifecycleError> {
        self.live_worker_mut(session)?;
        if self.pending_question().is_some() {
            return Err(LifecycleError::Stale);
        }
        // Il numero segue l'ultima domanda del tentativo: la cronologia scarta le
        // piu' vecchie e un id non deve ripetersi.
        let n =
            self.questions.iter().rev().find(|q| q.attempt == self.attempts).map_or(1, |q| question_number(&q.id) + 1);
        self.questions.push(TaskQuestion {
            id: format!("q-{}-{n}", self.attempts),
            attempt: self.attempts,
            text,
            options,
            asked_at: now,
            answered_at: None,
            answer: None,
        });
        if self.questions.len() > QUESTIONS_KEPT {
            self.questions.drain(..self.questions.len() - QUESTIONS_KEPT);
        }
        self.updated_at = now;
        Ok(())
    }

    pub(super) fn question_answered(
        &mut self,
        session: &str,
        id: &str,
        answer: Option<String>,
        now: i64,
    ) -> Result<(), LifecycleError> {
        self.live_worker_mut(session)?;
        let q = self.pending_with_id(id)?;
        q.answered_at = Some(now);
        q.answer = answer;
        *self.live_worker_mut(session)? = WorkerActivity::Running;
        self.updated_at = now;
        Ok(())
    }

    pub(super) fn answer_undelivered(&mut self, session: &str, id: &str, now: i64) -> Result<(), LifecycleError> {
        self.live_worker_mut(session)?;
        let attempt = self.attempts;
        let q = self
            .questions
            .last_mut()
            .filter(|q| q.id == id && q.attempt == attempt && q.answered_at.is_some() && q.answer.is_some())
            .ok_or(LifecycleError::Stale)?;
        q.answered_at = None;
        q.answer = None;
        *self.live_worker_mut(session)? = WorkerActivity::AwaitingAnswer;
        self.updated_at = now;
        Ok(())
    }
}

/// Il `<n>` di un id `q-<tentativo>-<n>` (0 se l'id non ha la forma attesa).
fn question_number(id: &str) -> u32 {
    id.rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests;
