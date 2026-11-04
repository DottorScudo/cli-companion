//! L'instradamento puro degli eventi di una board sui thread del canale del
//! workspace, raggruppati per obiettivo:
//!
//! - una proposta apre un thread (titolo dal primo task, "+N"), accettata ne
//!   collega i task, scartata lo chiude;
//! - un task creato a mano apre il suo thread;
//! - gli eventi di un task vanno nel thread a cui e' collegato; se non ce n'e'
//!   uno (task gia' esistente all'accensione dell'HUB) lo si crea al primo
//!   evento, tranne per un task eliminato (nessun thread solo per dirlo);
//! - un thread i cui task sono tutti conclusi (`done` o spariti) si chiude
//!   con un messaggio finale; un evento successivo di un suo task lo riapre.
//!
//! Tempo e id li passa il chiamante. Un evento che non si puo' riportare
//! finisce nel log e non ferma gli altri.

use super::diff::{BoardEvent, TaskChange};
use super::snapshot::{BoardSnapshot, ProposalSnap, TaskSnap};
use super::texts::{self, thread_title, PROPOSAL_REJECTED, THREAD_CONCLUDED};
use crate::board::TaskStatus;
use crate::hub::channel::Channel;
use crate::hub::model::{Author, HubEvent, HubEventKind, ThreadState, ThreadSummary};
use crate::hub::state::{MESSAGE_ID_PREFIX, THREAD_ID_PREFIX};
use crate::hub::threads::{summary, ChannelThreads, NewMessage};

/// Applica `events` (ricavati dalla board `board`) ai thread del canale e
/// ritorna i riassunti dei thread toccati, nell'ordine del primo tocco.
pub fn route(
    list: &mut ChannelThreads,
    channel: &Channel,
    board: &BoardSnapshot,
    events: &[BoardEvent],
    now: i64,
    next_id: &mut dyn FnMut(&'static str) -> String,
) -> Vec<ThreadSummary> {
    let mut router = Router { list, channel, board, now, next_id, touched: Vec::new() };
    for event in events {
        if let Err(e) = router.apply(event) {
            log::warn!("hub {}: evento della board su {} non riportato: {e}", board.workspace_id, subject(event));
        }
    }
    router.close_concluded();
    router.touched.iter().filter_map(|id| router.list.thread(id)).map(summary).collect()
}

struct Router<'a> {
    list: &'a mut ChannelThreads,
    channel: &'a Channel,
    board: &'a BoardSnapshot,
    now: i64,
    next_id: &'a mut dyn FnMut(&'static str) -> String,
    /// Thread modificati, senza duplicati.
    touched: Vec<String>,
}

impl Router<'_> {
    fn apply(&mut self, event: &BoardEvent) -> Result<(), String> {
        match event {
            BoardEvent::ProposalCreated(p) => self.proposal_created(p),
            BoardEvent::ProposalAccepted { proposal_id, task_ids } => self.proposal_accepted(proposal_id, task_ids),
            BoardEvent::ProposalRejected { proposal_id } => self.proposal_rejected(proposal_id),
            BoardEvent::Task { change, task } => self.task_event(*change, task),
        }
    }

    fn proposal_created(&mut self, p: &ProposalSnap) -> Result<(), String> {
        let thread = match self.thread_of_proposal(&p.id) {
            Some(id) => id,
            None => self.new_thread(thread_title(p.tasks.iter().map(|t| t.title.as_str())), |list, id, now| {
                list.link_proposal(id, &p.id, now).map(drop)
            })?,
        };
        let event = self.proposal_event(HubEventKind::ProposalCreated, &p.id);
        self.post(&thread, texts::proposal_created(p), Some(event))
    }

    /// Una proposta che non ha un thread (pendente gia' prima dell'accensione)
    /// lo riceve ora, intitolato con i suoi task.
    fn proposal_accepted(&mut self, proposal_id: &str, task_ids: &[String]) -> Result<(), String> {
        let thread = match self.thread_of_proposal(proposal_id) {
            Some(id) => id,
            None => {
                let titles: Vec<&str> =
                    task_ids.iter().filter_map(|id| self.board.task(id)).map(|t| t.title.as_str()).collect();
                self.new_thread(thread_title(titles.into_iter()), |list, id, now| {
                    list.link_proposal(id, proposal_id, now).map(drop)
                })?
            }
        };
        // Un task e' di un solo thread: quelli gia' altrove restano dove sono.
        let free: Vec<String> =
            task_ids.iter().filter(|id| self.list.thread_of_task(id).is_none_or(|t| t.id == thread)).cloned().collect();
        self.list.link_tasks(&thread, &free, self.now)?;
        self.reopen(&thread)?;
        let event = self.proposal_event(HubEventKind::ProposalAccepted, proposal_id);
        self.post(&thread, texts::proposal_accepted(task_ids.len()), Some(event))
    }

    fn proposal_rejected(&mut self, proposal_id: &str) -> Result<(), String> {
        let Some(thread) = self.thread_of_proposal(proposal_id) else {
            return Ok(());
        };
        let event = self.proposal_event(HubEventKind::ProposalRejected, proposal_id);
        self.post(&thread, PROPOSAL_REJECTED.to_string(), Some(event))?;
        self.list.set_state(&thread, ThreadState::Closed, self.now)?;
        Ok(())
    }

    fn task_event(&mut self, change: TaskChange, task: &TaskSnap) -> Result<(), String> {
        let thread = match (self.thread_of_task(&task.id), change) {
            (Some(id), _) => id,
            (None, TaskChange::Deleted) => return Ok(()),
            (None, _) => self.new_thread(thread_title(std::iter::once(task.title.as_str())), |list, id, now| {
                list.link_tasks(id, std::slice::from_ref(&task.id), now).map(drop)
            })?,
        };
        // Un task eliminato non riapre un obiettivo gia' concluso.
        if change != TaskChange::Deleted {
            self.reopen(&thread)?;
        }
        let event = self.task_hub_event(change, task);
        self.post(&thread, texts::task(change, task), Some(event))
    }

    /// Chiude, con un messaggio finale, i thread toccati e aperti i cui task
    /// sono tutti conclusi: `done` o spariti dalla board (eliminati o archiviati).
    fn close_concluded(&mut self) {
        for id in self.touched.clone() {
            let concluded = self.list.thread(&id).is_some_and(|t| {
                t.state == ThreadState::Open
                    && !t.task_ids.is_empty()
                    && t.task_ids.iter().all(|task| self.board.task(task).is_none_or(|s| s.status == TaskStatus::Done))
            });
            if !concluded {
                continue;
            }
            let closed = self
                .post(&id, THREAD_CONCLUDED.to_string(), None)
                .and_then(|()| self.list.set_state(&id, ThreadState::Closed, self.now).map(|_| ()));
            if let Err(e) = closed {
                log::warn!("hub {}: thread {id} non chiuso: {e}", self.board.workspace_id);
            }
        }
    }

    fn thread_of_task(&self, task_id: &str) -> Option<String> {
        self.list.thread_of_task(task_id).map(|t| t.id.clone())
    }

    fn thread_of_proposal(&self, proposal_id: &str) -> Option<String> {
        self.list.thread_of_proposal(proposal_id).map(|t| t.id.clone())
    }

    /// Nuovo thread collegato da `link` al suo soggetto: se il collegamento
    /// fallisce il thread sparisce, niente thread orfani.
    fn new_thread(
        &mut self,
        title: String,
        link: impl FnOnce(&mut ChannelThreads, &str, i64) -> Result<(), String>,
    ) -> Result<String, String> {
        let id = (self.next_id)(THREAD_ID_PREFIX);
        self.list.create_thread(self.channel, id.clone(), &title, self.now)?;
        if let Err(e) = link(self.list, &id, self.now) {
            self.list.threads.retain(|t| t.id != id);
            return Err(e);
        }
        self.touch(&id);
        Ok(id)
    }

    fn reopen(&mut self, thread: &str) -> Result<(), String> {
        self.list.set_state(thread, ThreadState::Open, self.now).map(|_| ())
    }

    fn post(&mut self, thread: &str, body: String, event: Option<HubEvent>) -> Result<(), String> {
        let new =
            NewMessage { id: (self.next_id)(MESSAGE_ID_PREFIX), author: Author::System, body, reply_to: None, event };
        self.list.add_message(thread, new, self.now)?;
        self.touch(thread);
        Ok(())
    }

    fn touch(&mut self, thread: &str) {
        if !self.touched.iter().any(|id| id == thread) {
            self.touched.push(thread.to_string());
        }
    }

    fn proposal_event(&self, kind: HubEventKind, proposal_id: &str) -> HubEvent {
        HubEvent { workspace: Some(self.board.workspace_id.clone()), ..HubEvent::of_proposal(kind, proposal_id) }
    }

    /// I dati del task: stato sempre; riassunto, verdetto e nota solo per un
    /// task concluso (sono quelli del tentativo appena finito).
    fn task_hub_event(&self, change: TaskChange, task: &TaskSnap) -> HubEvent {
        let concluded = matches!(change, TaskChange::Done | TaskChange::Failed);
        HubEvent {
            workspace: Some(self.board.workspace_id.clone()),
            status: Some(task.status.as_str().to_string()),
            summary: task.summary.clone().filter(|_| concluded),
            verdict: task.verdict.filter(|_| concluded),
            note: task.note.clone().filter(|_| concluded),
            ..HubEvent::of_task(event_kind(change), &task.id)
        }
    }
}

fn event_kind(change: TaskChange) -> HubEventKind {
    match change {
        TaskChange::Created => HubEventKind::TaskCreated,
        TaskChange::Started => HubEventKind::TaskStarted,
        TaskChange::Done => HubEventKind::TaskDone,
        TaskChange::Failed => HubEventKind::TaskFailed,
        TaskChange::Attention => HubEventKind::TaskAttention,
        TaskChange::Deleted => HubEventKind::TaskDeleted,
    }
}

/// Il soggetto dell'evento, per il log.
fn subject(event: &BoardEvent) -> &str {
    match event {
        BoardEvent::ProposalCreated(p) => &p.id,
        BoardEvent::ProposalAccepted { proposal_id, .. } | BoardEvent::ProposalRejected { proposal_id } => proposal_id,
        BoardEvent::Task { task, .. } => &task.id,
    }
}

#[cfg(test)]
mod tests;
