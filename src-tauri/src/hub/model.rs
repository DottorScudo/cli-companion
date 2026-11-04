//! Il modello dati dell'HUB: thread, messaggi, autori ed eventi. Sono i DTO
//! che attraversano `invoke`/`emit` e anche il formato su disco (dentro la
//! busta di `store`). Nei bindings hanno il prefisso `Hub` (`HubThread`...),
//! per non confondersi con i tipi generici del frontend.
//!
//! Compatibilita': i campi facoltativi hanno `#[serde(default)]`, cosi' un file
//! scritto prima che un campo esistesse resta leggibile. Aggiungere una variante
//! a un enum e' compatibile in lettura; rinominarla richiede una migrazione.

use super::channel::Channel;
use crate::board::Verdict;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubThreadState")]
pub enum ThreadState {
    Open,
    Closed,
}

/// Chi ha scritto un messaggio. `bot` e' un segretario (`name` = il suo handle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, rename = "HubAuthor")]
pub enum Author {
    User,
    System,
    Bot { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubMessageKind")]
pub enum MessageKind {
    /// Testo libero (dell'utente, di un segretario o un avviso dell'app).
    Text,
    /// Generato dall'app a partire da un evento della board: `event` e' presente.
    Event,
}

/// Tipo di un evento della board riportato in un thread (vedi `events`). Un
/// bot (tappa 2) li legge come dati.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubEventKind")]
pub enum HubEventKind {
    /// Task creato a mano, non da una proposta.
    TaskCreated,
    TaskStarted,
    TaskDone,
    TaskFailed,
    /// Il worker aspetta una risposta (stato `attention` della board).
    TaskAttention,
    /// Task tolto dalla board senza essere `done`.
    TaskDeleted,
    ProposalCreated,
    ProposalAccepted,
    ProposalRejected,
}

/// Di cosa parla un evento: l'attesa dell'utente si calcola per soggetto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subject<'a> {
    Task(&'a str),
    Proposal(&'a str),
}

impl HubEventKind {
    /// Lo stato che l'evento lascia aspetta una mossa dell'utente.
    pub fn waits_for_user(self) -> bool {
        matches!(self, Self::TaskFailed | Self::TaskAttention | Self::ProposalCreated)
    }

    fn is_about_proposal(self) -> bool {
        matches!(self, Self::ProposalCreated | Self::ProposalAccepted | Self::ProposalRejected)
    }
}

/// Dati strutturati di un evento, accanto al testo a template del messaggio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubEvent")]
pub struct HubEvent {
    pub kind: HubEventKind,
    /// Workspace della board da cui viene l'evento.
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub proposal_id: Option<String>,
    /// Stato del task nella board dopo l'evento (`done`, `failed`...).
    #[serde(default)]
    pub status: Option<String>,
    /// Riassunto breve (per esempio dal report del worker).
    #[serde(default)]
    pub summary: Option<String>,
    /// Esito di una revisione completata.
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// Nota dell'app sul task (per esempio il motivo di un fallimento).
    #[serde(default)]
    pub note: Option<String>,
}

impl HubEvent {
    /// Evento di un task, senza gli altri dati.
    pub fn of_task(kind: HubEventKind, task_id: &str) -> Self {
        Self { task_id: Some(task_id.to_string()), ..Self::new(kind) }
    }

    /// Evento di una proposta, senza gli altri dati.
    pub fn of_proposal(kind: HubEventKind, proposal_id: &str) -> Self {
        Self { proposal_id: Some(proposal_id.to_string()), ..Self::new(kind) }
    }

    /// Evento senza dati: il chiamante deve impostarne almeno il soggetto.
    pub fn new(kind: HubEventKind) -> Self {
        Self {
            kind,
            workspace: None,
            task_id: None,
            proposal_id: None,
            status: None,
            summary: None,
            verdict: None,
            note: None,
        }
    }

    /// Il soggetto dell'evento: il task per gli eventi dei task, la proposta per
    /// quelli delle proposte. `None` se manca l'id che il tipo richiede.
    pub fn subject(&self) -> Option<Subject<'_>> {
        if self.kind.is_about_proposal() {
            self.proposal_id.as_deref().map(Subject::Proposal)
        } else {
            self.task_id.as_deref().map(Subject::Task)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubMessage")]
pub struct Message {
    pub id: String,
    pub author: Author,
    pub kind: MessageKind,
    pub body: String,
    /// Messaggio dello stesso thread a cui risponde (puo' essere gia' stato
    /// potato dai limiti: la UI lo tratta come assente).
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub event: Option<HubEvent>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubThread")]
pub struct Thread {
    pub id: String,
    #[ts(type = "string")]
    pub channel: Channel,
    pub title: String,
    pub state: ThreadState,
    #[serde(default)]
    pub task_ids: Vec<String>,
    #[serde(default)]
    pub proposal_id: Option<String>,
    /// Il segretario titolare del thread: quello che vi ha aperto l'ultimo task
    /// di pianificazione (`open_plan_task`). Alcuni eventi della board lo
    /// svegliano (vedi `events::route`). Assente nei thread senza
    /// pianificazione e nei file scritti prima.
    #[serde(default)]
    pub owner: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Riassunto del thread scritto da un segretario alla chiusura della sua
    /// sessione (poche righe, vedi `engine::context::clean_summary`); lo legge
    /// il segretario che si sveglia dopo.
    #[serde(default)]
    pub summary: Option<String>,
    /// In ordine di arrivo, dal piu' vecchio.
    #[serde(default)]
    pub messages: Vec<Message>,
}

/// Un thread senza i messaggi: la riga dell'elenco e il payload di `hub-update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "HubThreadSummary")]
pub struct ThreadSummary {
    pub id: String,
    #[ts(type = "string")]
    pub channel: Channel,
    pub title: String,
    pub state: ThreadState,
    pub task_count: usize,
    pub message_count: usize,
    /// C'e' un evento che aspetta l'utente: proposta pendente, task fallito o
    /// in attenzione (vedi `waiting::waiting_user`).
    pub waiting_user: bool,
    /// Il segretario titolare del thread (vedi `Thread::owner`).
    pub owner: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
