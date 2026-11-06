//! Ciclo di vita e collegamenti dei thread: chiusura e riapertura,
//! eliminazione, task e proposte collegati e la ricerca del thread di un task
//! o di una proposta. Li usa l'instradamento degli eventi della board
//! (`events::route`).

use super::{text, ChannelThreads, THREAD_NOT_FOUND};
use crate::board::{valid_id, MAX_PENDING_PROPOSALS};
use crate::hub::model::{Thread, ThreadState};

/// Thread chiusi tenuti per canale: oltre, si tolgono i chiusi piu' vecchi.
pub const CLOSED_THREADS_MAX: usize = 200;
/// Task collegabili a un thread.
pub const TASKS_PER_THREAD_MAX: usize = 200;
/// Proposte collegabili a un thread: quante ne puo' avere pendenti una board
/// (le decise si tolgono, vedi `unlink_proposal`).
pub const PROPOSALS_PER_THREAD_MAX: usize = MAX_PENDING_PROPOSALS;

impl ChannelThreads {
    /// Il thread a cui e' collegato il task.
    pub fn thread_of_task(&self, task_id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.task_ids.iter().any(|id| id == task_id))
    }

    /// Il thread a cui e' collegata la proposta.
    pub fn thread_of_proposal(&self, proposal_id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.proposal_ids.iter().any(|id| id == proposal_id))
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

    /// Toglie il thread dal canale, in qualunque stato. I task e la proposta
    /// collegati restano sulla board: un loro evento successivo, non trovando
    /// piu' un thread, ne apre uno nuovo. Torna il thread tolto.
    pub fn remove_thread(&mut self, thread_id: &str) -> Result<Thread, String> {
        let at = self.threads.iter().position(|t| t.id == thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
        Ok(self.threads.remove(at))
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

    /// Collega la proposta al thread, accanto alle altre. Una proposta appartiene
    /// a un solo thread. Oltre `PROPOSALS_PER_THREAD_MAX` si tolgono le piu'
    /// vecchie: proposte decise mentre l'HUB era spento, di cui il thread non
    /// ha visto la decisione.
    pub fn link_proposal(&mut self, thread_id: &str, proposal_id: &str, now: i64) -> Result<&Thread, String> {
        if !text::valid_ref_id(proposal_id) {
            return Err("id della proposta non valido".to_string());
        }
        self.thread_mut(thread_id)?;
        if self.thread_of_proposal(proposal_id).is_some_and(|t| t.id != thread_id) {
            return Err("la proposta è già collegata a un altro thread".to_string());
        }
        let thread = self.thread_mut(thread_id)?;
        if !thread.proposal_ids.iter().any(|id| id == proposal_id) {
            thread.proposal_ids.push(proposal_id.to_string());
            let excess = thread.proposal_ids.len().saturating_sub(PROPOSALS_PER_THREAD_MAX);
            thread.proposal_ids.drain(..excess);
            thread.updated_at = now;
        }
        Ok(thread)
    }

    /// Toglie la proposta, ormai decisa, dal thread a cui era collegata. Non
    /// cambia `updated_at`: il thread lo tocca il messaggio della decisione.
    pub fn unlink_proposal(&mut self, proposal_id: &str) {
        for thread in &mut self.threads {
            thread.proposal_ids.retain(|id| id != proposal_id);
        }
    }

    /// Rende il segretario `owner` titolare del thread (sostituisce il precedente).
    pub fn set_owner(&mut self, thread_id: &str, owner: &str, now: i64) -> Result<&Thread, String> {
        let owner = text::bot_name(owner)?;
        let thread = self.thread_mut(thread_id)?;
        if thread.owner.as_deref() != Some(owner.as_str()) {
            thread.owner = Some(owner);
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
