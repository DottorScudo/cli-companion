//! I tool di pianificazione dei segretari (puro, generico sulla board):
//!
//! - `open_plan_task`: il task `plan` (titolo dal goal, testo in inglese con la
//!   richiesta dell'utente citata testualmente, il riassunto del thread e gli
//!   ultimi messaggi, e gli allegati con il percorso assoluto), collegato al
//!   thread, con il segretario come titolare;
//! - `decide_proposal`: senza id. La proposta e' quella dell'evento a cui
//!   risponde (`reply_to`) il messaggio dell'utente che ha svegliato il turno
//!   (`proposal_target`), ancora pendente; un turno svegliato da un evento
//!   della board non ne ha mai una. La decisione resta dell'utente: il codice
//!   garantisce che solo una sua risposta esplicita a quell'evento la renda
//!   possibile.
//!
//! Gli errori sono in inglese: tornano al modello.

use super::sessions::WakeCause;
use crate::hub::actions::PlanOrder;
use crate::hub::attachments::listing::AttachmentPaths;
use crate::hub::channel::Channel;
use crate::hub::model::{Author, HubEventKind, Message, Subject, Thread};
use crate::hub::text::clean_line;
use crate::hub::threads::{ChannelThreads, TASKS_PER_THREAD_MAX};
use crate::hub::waiting::subject_waiting;
use crate::validate::clip_with_ellipsis;
use serde_json::{json, Value};

mod attachments;

/// Caratteri del goal di `open_plan_task`.
pub const GOAL_MAX: usize = 4000;
/// Caratteri tenuti del titolo del task `plan` (la board ne ammette 80; il
/// taglio aggiunge "…").
const TITLE_KEPT: usize = 72;
/// Titolo di riserva se la prima riga del goal non lascia nulla.
const FALLBACK_TITLE: &str = "Plan";
/// Messaggi del thread riportati nel testo del piano, prima di quello del turno.
const CONTEXT_MESSAGES: usize = 10;
/// Caratteri tenuti di ognuno di quei messaggi.
const CONTEXT_ENTRY_KEPT: usize = 1000;

const GENERAL_CHANNEL: &str = "This thread is in the General channel, which belongs to no project. Ask the user \
which workspace the work is for, and to ask again in that workspace's channel.";
const THREAD_GONE: &str = "This thread no longer exists.";
const THREAD_FULL: &str = "This thread already has the maximum number of linked tasks: ask the user to continue in a \
new thread.";
const NOT_A_REPLY: &str = "decide_proposal works only when the user's message that woke you up is a reply to a \
pending proposal event of this thread, never when a board event woke you up. Do not decide: tell the user to use \
Reply on the proposal message and say whether to accept or reject it.";
const NOT_PENDING: &str = "That proposal is no longer pending: it was already accepted or rejected. Tell the user.";

/// La proposta a cui risponde il messaggio che ha svegliato il turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalRef {
    pub workspace: String,
    pub proposal_id: String,
}

/// La board vista da `decide_proposal`.
pub trait ProposalBoard {
    /// Quanti task ha la proposta, se e' ancora pendente sulla board.
    fn pending_tasks(&self, workspace: &str, proposal_id: &str) -> Option<usize>;
    /// Accetta o scarta la proposta (con la validazione della board).
    fn decide(&self, workspace: &str, proposal_id: &str, accept: bool) -> Result<(), String>;
}

/// La proposta che il turno puo' decidere: solo per un turno svegliato da un
/// messaggio dell'utente (`cause`) che risponde a un evento "proposta" del
/// thread, del workspace del thread, ancora pendente nel thread.
pub fn proposal_target(thread: &Thread, trigger_id: &str, cause: WakeCause) -> Option<ProposalRef> {
    if cause != WakeCause::Message {
        return None;
    }
    let trigger = thread.messages.iter().find(|m| m.id == trigger_id && m.author == Author::User)?;
    let target_id = trigger.reply_to.as_deref()?;
    let event = thread.messages.iter().find(|m| m.id == target_id)?.event.as_ref()?;
    let proposal_id = event.proposal_id.as_deref().filter(|_| event.kind == HubEventKind::ProposalCreated)?;
    let workspace = event.workspace.as_deref()?;
    let of_this_channel = thread.channel == Channel::Workspace(workspace.to_string());
    (of_this_channel && subject_waiting(&thread.messages, Subject::Proposal(proposal_id)))
        .then(|| ProposalRef { workspace: workspace.to_string(), proposal_id: proposal_id.to_string() })
}

/// `decide_proposal`: decide la proposta del turno, se c'e' ed e' ancora
/// pendente sulla board. Il messaggio nel thread ("Proposta accettata: N task
/// in coda.") lo scrivono gli eventi della board.
pub fn decide(board: &impl ProposalBoard, target: Option<&ProposalRef>, accept: bool) -> Result<Value, String> {
    let target = target.ok_or_else(|| NOT_A_REPLY.to_string())?;
    let tasks = board.pending_tasks(&target.workspace, &target.proposal_id).ok_or_else(|| NOT_PENDING.to_string())?;
    let decision = if accept { "accepted" } else { "rejected" };
    board.decide(&target.workspace, &target.proposal_id, accept).map_err(|e| {
        log::warn!("hub: proposta {} non decisa dal segretario: {e}", target.proposal_id);
        format!("The proposal could not be {decision}: {e}")
    })?;
    let queued = if accept { tasks } else { 0 };
    Ok(json!({ "proposal": target.proposal_id, "decision": decision, "tasks_queued": queued }))
}

/// Una chiamata a `open_plan_task`: il thread del turno, il messaggio che lo
/// ha svegliato, il segretario e il goal validato.
pub struct PlanTicket<'a> {
    pub channel: &'a Channel,
    pub thread_id: &'a str,
    pub trigger_id: &'a str,
    pub secretary: &'a str,
    pub goal: &'a str,
    /// Gli allegati (`HubDirs::attachments`) e l'ambiente del workspace del piano,
    /// da cui ricavare i percorsi che legge il pianificatore.
    pub attachments: AttachmentPaths<'a>,
}

/// `open_plan_task` sui thread del canale: crea il task `plan` con `create`
/// (workspace, ordine), lo collega al thread e rende il segretario titolare.
/// Nel Generale, o con il thread pieno, non crea nulla.
pub fn open_plan(
    list: &mut ChannelThreads,
    ticket: &PlanTicket<'_>,
    now: i64,
    create: impl FnOnce(&str, &PlanOrder) -> Result<String, String>,
) -> Result<Value, String> {
    let Channel::Workspace(workspace) = ticket.channel else {
        return Err(GENERAL_CHANNEL.to_string());
    };
    let thread = list.thread(ticket.thread_id).ok_or_else(|| THREAD_GONE.to_string())?;
    if thread.task_ids.len() >= TASKS_PER_THREAD_MAX {
        return Err(THREAD_FULL.to_string());
    }
    let order = plan_order(ticket.goal, thread, ticket.trigger_id, ticket.secretary, ticket.attachments);
    let task_id = create(workspace, &order).map_err(|e| format!("The planning task could not be created: {e}"))?;
    let attached = list
        .link_tasks(ticket.thread_id, std::slice::from_ref(&task_id), now)
        .map(drop)
        .and_then(|()| list.set_owner(ticket.thread_id, ticket.secretary, now).map(drop));
    if let Err(e) = attached {
        log::error!(
            "hub: task di pianificazione {task_id} creato ma non collegato al thread {}: {e}",
            ticket.thread_id
        );
        return Err(format!("The planning task {task_id} was created but could not be linked to this thread."));
    }
    Ok(json!({
        "task_id": task_id,
        "title": order.title,
        "status": "queued",
        "note": "You are now the owner of this thread: you will be woken up when the proposal is ready, if the \
            planner or a worker asks a question, if a task fails or needs attention, and when all tasks are concluded."
    }))
}

/// Titolo e testo del task `plan`. Il testo e' in inglese (lo legge il
/// pianificatore) e cita la richiesta dell'utente cosi' com'e'; gli allegati
/// della richiesta e dei messaggi recenti hanno la loro sezione, con i percorsi
/// di `attachments`.
pub fn plan_order(
    goal: &str,
    thread: &Thread,
    trigger_id: &str,
    secretary: &str,
    attachments: AttachmentPaths<'_>,
) -> PlanOrder {
    let first_line = goal.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let title = clean_line(first_line, TITLE_KEPT);
    let title = if title.is_empty() { FALLBACK_TITLE.to_string() } else { title };
    let at = thread.messages.iter().position(|m| m.id == trigger_id).unwrap_or(thread.messages.len());
    let request_message = user_request(&thread.messages, at);
    let request = request_message.map(|m| quoted(&m.body)).unwrap_or_else(|| "(no message from the user)".to_string());
    let before = &thread.messages[..at];
    let recent_messages = &before[before.len().saturating_sub(CONTEXT_MESSAGES)..];
    let recent: Vec<String> = recent_messages.iter().map(context_line).collect();
    let recent = if recent.is_empty() { "(none)".to_string() } else { recent.join("\n") };
    let attachments = attachments::section(attachments, thread, request_message, recent_messages);
    let task = format!(
        "Plan the following request. The user made it in a thread of the Agents HUB of cli-companion, and the \
secretary @{secretary} handed it over to you.\n\n## User request (verbatim)\n\n{request}\n\n{attachments}## Goal\n\n\
{goal}\n\n## Thread summary\n\n{summary}\n\n## Recent messages of the thread (oldest first)\n\n{recent}\n",
        summary = thread.summary.as_deref().unwrap_or("(none)"),
    );
    PlanOrder { title, task }
}

/// Il messaggio dell'utente che ha chiesto il lavoro: quello del turno se e'
/// dell'utente, altrimenti l'ultimo dell'utente prima (un turno svegliato da un
/// evento).
fn user_request(messages: &[Message], at: usize) -> Option<&Message> {
    let end = (at + 1).min(messages.len());
    messages[..end].iter().rev().find(|m| m.author == Author::User)
}

fn quoted(body: &str) -> String {
    body.lines().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n")
}

fn context_line(m: &Message) -> String {
    let author = match &m.author {
        Author::User => "user".to_string(),
        Author::System => "app".to_string(),
        Author::Bot { name } => format!("@{name}"),
    };
    let body = clip_with_ellipsis(&m.body, CONTEXT_ENTRY_KEPT).replace('\n', " / ");
    format!("- [{author}] {body}{}", attached_names(m))
}

/// ` [attached: a.png, b.txt]` (i percorsi sono nella sezione "Attachments"), o
/// niente se il messaggio non ha allegati.
fn attached_names(m: &Message) -> String {
    if m.attachments.is_empty() {
        return String::new();
    }
    let names: Vec<&str> = m.attachments.iter().map(|a| a.name.as_str()).collect();
    format!(" [attached: {}]", names.join(", "))
}

#[cfg(test)]
mod tests;
