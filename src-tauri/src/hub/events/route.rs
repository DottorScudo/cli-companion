//! L'instradamento puro degli eventi di una board sui thread del canale del
//! workspace, raggruppati per obiettivo:
//!
//! - una proposta apre un thread (titolo dal primo task, "+N"), accettata ne
//!   collega i task, scartata lo chiude. La proposta di un task `plan` o
//!   `review` gia' in un thread va in quel thread (lo riapre se chiuso), non in
//!   uno nuovo; ripiega sul thread nuovo se il thread ha un'altra proposta
//!   pendente o non ha posto per i task. Scartata in un thread con altri task
//!   oltre ai `plan`, non lo chiude: lo fa la conclusione dei task;
//! - un task creato a mano apre il suo thread;
//! - gli eventi di un task vanno nel thread a cui e' collegato; se non ce n'e'
//!   uno (task gia' esistente all'accensione dell'HUB) lo si crea al primo
//!   evento, tranne per un task eliminato (nessun thread solo per dirlo);
//! - un thread i cui task sono tutti conclusi (`done` o spariti) si chiude
//!   con un messaggio finale; un evento successivo di un suo task lo riapre.
//!   I task `plan` non bastano a chiuderlo e una proposta pendente lo tiene aperto.
//!
//! Nei thread con un titolare (`Thread::owner`) alcuni eventi lo svegliano
//! (`EventWake`): proposta pronta, pianificazione conclusa senza proposta,
//! domanda di un worker o del pianificatore, task fallito o in attenzione,
//! tutti i task conclusi. Avvio e completamento
//! di un singolo task restano solo messaggi: un risveglio costa una chiamata al
//! modello. Al massimo un risveglio per thread a ogni passo, sull'ultimo
//! evento che lo chiede (il contesto del risveglio porta anche i precedenti).
//!
//! Tempo e id li passa il chiamante. Un evento che non si puo' riportare
//! finisce nel log e non ferma gli altri.

use super::diff::{BoardEvent, TaskChange};
use super::notices::{self, EventNotice};
use super::snapshot::{BoardSnapshot, ProposalSnap, TaskSnap};
use super::texts::{self, thread_title, PROPOSAL_REJECTED, THREAD_CONCLUDED};
use crate::board::{TaskKind, TaskStatus};
use crate::hub::channel::Channel;
use crate::hub::engine::confirm;
use crate::hub::model::{Author, HubEvent, HubEventKind, Thread, ThreadState, ThreadSummary};
use crate::hub::notify::NoticeKind;
use crate::hub::state::{MESSAGE_ID_PREFIX, THREAD_ID_PREFIX};
use crate::hub::threads::{summary, ChannelThreads, NewMessage, TASKS_PER_THREAD_MAX};

/// Un segretario titolare da svegliare per un evento della board: il suo
/// turno parte dal messaggio `trigger` del thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventWake {
    pub thread_id: String,
    pub secretary: String,
    pub trigger: String,
}

/// L'esito di un passo: i thread toccati (nell'ordine del primo tocco), i
/// titolari da svegliare e le notifiche di sistema da dare (vedi `notices`).
#[derive(Debug, Default)]
pub struct Routed {
    pub touched: Vec<ThreadSummary>,
    pub wakes: Vec<EventWake>,
    pub notices: Vec<EventNotice>,
}

/// Applica `events` (ricavati dalla board `board`) ai thread del canale.
pub fn route(
    list: &mut ChannelThreads,
    channel: &Channel,
    board: &BoardSnapshot,
    events: &[BoardEvent],
    now: i64,
    next_id: &mut dyn FnMut(&'static str) -> String,
) -> Routed {
    let mut router =
        Router { list, channel, board, now, next_id, touched: Vec::new(), wakes: Vec::new(), notices: Vec::new() };
    for event in events {
        if let Err(e) = router.apply(event) {
            log::warn!("hub {}: evento della board su {} non riportato: {e}", board.workspace_id, subject(event));
        }
    }
    router.close_concluded();
    let touched = router.touched.iter().filter_map(|id| router.list.thread(id)).map(summary).collect();
    Routed { touched, wakes: router.wakes, notices: router.notices }
}

struct Router<'a> {
    list: &'a mut ChannelThreads,
    channel: &'a Channel,
    board: &'a BoardSnapshot,
    now: i64,
    next_id: &'a mut dyn FnMut(&'static str) -> String,
    /// Thread modificati, senza duplicati.
    touched: Vec<String>,
    /// Al massimo uno per thread.
    wakes: Vec<EventWake>,
    /// Una per messaggio che notifica; il raggruppamento e' di chi le riceve.
    notices: Vec<EventNotice>,
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
            None => match self.origin_thread(p) {
                Some(id) => {
                    self.list.link_proposal(&id, &p.id, self.now)?;
                    // Una proposta pendente tiene aperto anche un obiettivo gia' concluso.
                    self.reopen(&id)?;
                    id
                }
                None => self.new_thread(thread_title(p.tasks.iter().map(|t| t.title.as_str())), |list, id, now| {
                    list.link_proposal(id, &p.id, now).map(drop)
                })?,
            },
        };
        let event =
            HubEvent { task_id: p.source_task.clone(), ..self.proposal_event(HubEventKind::ProposalCreated, &p.id) };
        let message =
            self.post_noticing(&thread, texts::proposal_created(p), Some(event), Some(NoticeKind::ProposalCreated))?;
        self.wake_owner(&thread, message);
        Ok(())
    }

    /// Il thread d'origine della proposta: quello del task (`plan` o `review`)
    /// che l'ha fatta. `None`, e la proposta apre un thread nuovo, se non c'e'
    /// un task sorgente (Intake), se il task non ha thread, se il thread ha gia'
    /// un'altra proposta pendente (lo slot e' unico) o se i task proposti non
    /// ci stanno piu'.
    fn origin_thread(&self, p: &ProposalSnap) -> Option<String> {
        let source = p.source_task.as_deref()?;
        let thread = self.list.thread_of_task(source)?;
        let other_pending = thread
            .proposal_id
            .as_deref()
            .is_some_and(|id| id != p.id && self.board.proposals.iter().any(|q| q.id == id));
        let fits = thread.task_ids.len() + p.tasks.len() <= TASKS_PER_THREAD_MAX;
        (!other_pending && fits).then(|| thread.id.clone())
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
        self.post(&thread, texts::proposal_accepted(task_ids.len()), Some(event)).map(drop)
    }

    fn proposal_rejected(&mut self, proposal_id: &str) -> Result<(), String> {
        let Some(thread) = self.thread_of_proposal(proposal_id) else {
            return Ok(());
        };
        let event = self.proposal_event(HubEventKind::ProposalRejected, proposal_id);
        self.post(&thread, PROPOSAL_REJECTED.to_string(), Some(event))?;
        // Un thread con altri task e' un obiettivo in corso: lo chiude
        // `close_concluded` solo quando sono tutti conclusi.
        if !self.has_work_tasks(&thread) {
            self.list.set_state(&thread, ThreadState::Closed, self.now)?;
        }
        Ok(())
    }

    /// Il thread ha task diversi dai `plan` (uno sparito dalla board conta come tale).
    fn has_work_tasks(&self, thread: &str) -> bool {
        self.list
            .thread(thread)
            .is_some_and(|t| t.task_ids.iter().any(|id| self.board.task(id).is_none_or(|t| t.kind != TaskKind::Plan)))
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
        let unanswered_plan =
            change == TaskChange::Done && task.kind == TaskKind::Plan && !self.proposed(&thread, task);
        let parts = if unanswered_plan { texts::plan_without_proposal(task) } else { vec![texts::task(change, task)] };
        let event = self.task_hub_event(change, task);
        let notice = notices::for_task(change, task.kind, unanswered_plan);
        let message = self.post_parts(&thread, parts, event, notice)?;
        self.invalidate_requests(&thread, change, task);
        if unanswered_plan || matches!(change, TaskChange::Failed | TaskChange::Attention | TaskChange::Question) {
            self.wake_owner(&thread, message);
        }
        Ok(())
    }

    /// Le richieste di azione in attesa del task che non valgono piu' perche' ha
    /// cambiato stato o e' sparito diventano `stale` (vedi `confirm`).
    fn invalidate_requests(&mut self, thread: &str, change: TaskChange, task: &TaskSnap) {
        let status = (change != TaskChange::Deleted).then(|| task.status.as_str());
        if let Err(e) = confirm::invalidate_for_task(self.list, thread, &task.id, status, self.now) {
            log::warn!("hub {}: richieste del task {} non invalidate: {e}", self.board.workspace_id, task.id);
        }
    }

    /// Il task `plan` ha fatto una proposta: e' nella board o e' gia' stata
    /// riportata nel thread (decisa prima della fine del piano).
    fn proposed(&self, thread: &str, plan: &TaskSnap) -> bool {
        let source = Some(plan.id.as_str());
        self.board.proposals.iter().any(|p| p.source_task.as_deref() == source)
            || self.list.thread(thread).is_some_and(|t| {
                t.messages
                    .iter()
                    .filter_map(|m| m.event.as_ref())
                    .any(|e| e.kind == HubEventKind::ProposalCreated && e.task_id.as_deref() == source)
            })
    }

    /// Chiude, con un messaggio finale, i thread toccati e aperti i cui task
    /// sono tutti conclusi (vedi `concluded`); il titolare si sveglia per
    /// riassumere.
    fn close_concluded(&mut self) {
        for id in self.touched.clone() {
            if !self.list.thread(&id).is_some_and(|t| self.concluded(t)) {
                continue;
            }
            let closed = self
                .post_noticing(&id, THREAD_CONCLUDED.to_string(), None, Some(NoticeKind::ThreadConcluded))
                .and_then(|message| {
                    self.list.set_state(&id, ThreadState::Closed, self.now)?;
                    Ok(message)
                });
            match closed {
                Ok(message) => self.wake_owner(&id, message),
                Err(e) => log::warn!("hub {}: thread {id} non chiuso: {e}", self.board.workspace_id),
            }
        }
    }

    /// Thread aperto, senza una proposta pendente, con almeno un task che non
    /// e' un `plan` (un piano da solo non e' un obiettivo concluso: aspetta la
    /// proposta o le risposte) e tutti i suoi task nella board `done`. Un task
    /// sparito dalla board (eliminato o archiviato) conta come concluso.
    fn concluded(&self, thread: &Thread) -> bool {
        let pending = thread.proposal_id.as_deref().is_some_and(|p| self.board.proposals.iter().any(|q| q.id == p));
        let tasks: Vec<Option<&TaskSnap>> = thread.task_ids.iter().map(|id| self.board.task(id)).collect();
        thread.state == ThreadState::Open
            && !pending
            && tasks.iter().any(|t| t.is_none_or(|t| t.kind != TaskKind::Plan))
            && tasks.iter().all(|t| t.is_none_or(|t| t.status == TaskStatus::Done))
    }

    /// Il titolare del thread, se ne ha uno, si sveglia dal messaggio
    /// `trigger`; un risveglio gia' chiesto nello stesso passo parte
    /// dall'ultimo evento.
    fn wake_owner(&mut self, thread: &str, trigger: String) {
        let Some(secretary) = self.list.thread(thread).and_then(|t| t.owner.clone()) else {
            return;
        };
        match self.wakes.iter_mut().find(|w| w.thread_id == thread) {
            Some(wake) => (wake.secretary, wake.trigger) = (secretary, trigger),
            None => self.wakes.push(EventWake { thread_id: thread.to_string(), secretary, trigger }),
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

    /// Scrive il messaggio di sistema e ritorna il suo id.
    fn post(&mut self, thread: &str, body: String, event: Option<HubEvent>) -> Result<String, String> {
        self.post_noticing(thread, body, event, None)
    }

    /// Come `post`; con `notice` il messaggio e' anche una notifica di sistema,
    /// con il suo corpo come testo.
    fn post_noticing(
        &mut self,
        thread: &str,
        body: String,
        event: Option<HubEvent>,
        notice: Option<NoticeKind>,
    ) -> Result<String, String> {
        let id = (self.next_id)(MESSAGE_ID_PREFIX);
        let text = notice.as_ref().map(|_| body.clone());
        let new = NewMessage { id: id.clone(), author: Author::System, body, reply_to: None, event, origin: None };
        self.list.add_message(thread, new, self.now)?;
        self.touch(thread);
        if let (Some(kind), Some(text)) = (notice, text) {
            self.notices.push(EventNotice { thread_id: thread.to_string(), message_id: id.clone(), kind, text });
        }
        Ok(id)
    }

    /// Le parti di un testo lungo (`text::parts`) come messaggi consecutivi:
    /// la prima porta l'evento e l'eventuale notifica, le altre solo il testo.
    /// Ritorna l'id dell'ultima, da cui il titolare si sveglia vedendole tutte.
    fn post_parts(
        &mut self,
        thread: &str,
        parts: Vec<String>,
        event: HubEvent,
        notice: Option<NoticeKind>,
    ) -> Result<String, String> {
        let mut parts = parts.into_iter();
        let first = parts.next().ok_or_else(|| "il messaggio dell'evento è vuoto".to_string())?;
        let mut last = self.post_noticing(thread, first, Some(event), notice)?;
        for part in parts {
            last = self.post(thread, part, None)?;
        }
        Ok(last)
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
    /// task concluso (sono quelli del tentativo appena finito); la domanda
    /// solo per gli eventi della domanda.
    fn task_hub_event(&self, change: TaskChange, task: &TaskSnap) -> HubEvent {
        let concluded = matches!(change, TaskChange::Done | TaskChange::Failed);
        let about_question = matches!(change, TaskChange::Question | TaskChange::QuestionAnswered);
        HubEvent {
            question: task.question.clone().filter(|_| about_question),
            workspace: Some(self.board.workspace_id.clone()),
            status: Some(task.status.as_str().to_string()),
            summary: task.summary.clone().filter(|_| concluded),
            summary_chars: task.summary_chars.filter(|_| concluded),
            verdict: task.verdict.filter(|_| concluded),
            note: task.note.clone().filter(|_| concluded),
            note_chars: task.note_chars.filter(|_| concluded),
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
        TaskChange::Question => HubEventKind::TaskQuestion,
        TaskChange::QuestionAnswered => HubEventKind::TaskQuestionAnswered,
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
mod long_text_tests;
#[cfg(test)]
mod notice_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod tests;
