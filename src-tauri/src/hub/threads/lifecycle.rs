//! Ciclo di vita e collegamenti dei thread: chiusura e riapertura, task e
//! proposte collegati e la ricerca del thread di un task o di una proposta.
//! Li usa l'instradamento degli eventi della board (`events::route`).

use super::{text, ChannelThreads, THREAD_NOT_FOUND};
use crate::board::valid_id;
use crate::hub::model::{Thread, ThreadState};

/// Thread chiusi tenuti per canale: oltre, si tolgono i chiusi piu' vecchi.
pub const CLOSED_THREADS_MAX: usize = 200;
/// Task collegabili a un thread.
pub const TASKS_PER_THREAD_MAX: usize = 200;

impl ChannelThreads {
    /// Il thread a cui e' collegato il task.
    pub fn thread_of_task(&self, task_id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.task_ids.iter().any(|id| id == task_id))
    }

    /// Il thread a cui e' collegata la proposta.
    pub fn thread_of_proposal(&self, proposal_id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.proposal_id.as_deref() == Some(proposal_id))
    }

    /// Chiude o riapre il thread. Alla chiusura il canale tiene al massimo
    /// `CLOSED_THREADS_MAX` thread chiusi: si tolgono quelli aggiornati da piu'
    /// tempo (mai quello appena chiuso).
    pub fn set_state(&mut self, thread_id: &str, state: ThreadState, now: i64) -> Result<&Thread, String> {
        let thread = self.thread_mut(thread_id)?;
        if thread.state != state {
            thread.state = state;
            thread.updated_at = now;
        }
        if state == ThreadState::Closed {
            self.prune_closed(thread_id);
        }
        self.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())
    }

    /// Collega i task al thread (senza duplicati). Un task appartiene a un solo
    /// thread: se e' gia' collegato a un altro, nulla cambia e torna un errore.
    pub fn link_tasks(&mut self, thread_id: &str, task_ids: &[String], now: i64) -> Result<&Thread, String> {
        if let Some(bad) = task_ids.iter().find(|id| !valid_id(id)) {
            return Err(format!("id del task non valido: {}", crate::validate::clip(bad)));
        }
        self.thread_mut(thread_id)?;
        if let Some(taken) = task_ids.iter().find(|id| self.thread_of_task(id).is_some_and(|t| t.id != thread_id)) {
            return Err(format!("il task {taken} è già collegato a un altro thread"));
        }
        let thread = self.thread_mut(thread_id)?;
        let mut linked = thread.task_ids.clone();
        for id in task_ids {
            if !linked.contains(id) {
                linked.push(id.clone());
            }
        }
        if linked.len() > TASKS_PER_THREAD_MAX {
            return Err(format!("un thread può avere al massimo {TASKS_PER_THREAD_MAX} task"));
        }
        if linked != thread.task_ids {
            thread.task_ids = linked;
            thread.updated_at = now;
        }
        Ok(thread)
    }

    /// Collega la proposta al thread. Una proposta appartiene a un solo thread.
    pub fn link_proposal(&mut self, thread_id: &str, proposal_id: &str, now: i64) -> Result<&Thread, String> {
        if !text::valid_ref_id(proposal_id) {
            return Err("id della proposta non valido".to_string());
        }
        self.thread_mut(thread_id)?;
        if self.thread_of_proposal(proposal_id).is_some_and(|t| t.id != thread_id) {
            return Err("la proposta è già collegata a un altro thread".to_string());
        }
        let thread = self.thread_mut(thread_id)?;
        if thread.proposal_id.as_deref() != Some(proposal_id) {
            thread.proposal_id = Some(proposal_id.to_string());
            thread.updated_at = now;
        }
        Ok(thread)
    }

    fn prune_closed(&mut self, keep: &str) {
        let mut closed: Vec<(i64, String)> = self
            .threads
            .iter()
            .filter(|t| t.state == ThreadState::Closed && t.id != keep)
            .map(|t| (t.updated_at, t.id.clone()))
            .collect();
        // Il thread appena chiuso occupa uno dei posti.
        let excess = (closed.len() + 1).saturating_sub(CLOSED_THREADS_MAX);
        if excess == 0 {
            return;
        }
        closed.sort();
        let drop: Vec<String> = closed.into_iter().take(excess).map(|(_, id)| id).collect();
        self.threads.retain(|t| !drop.contains(&t.id));
    }
}
