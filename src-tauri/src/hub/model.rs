//! Il modello dati dell'HUB: thread, messaggi, autori ed eventi. Sono i DTO
//! che attraversano `invoke`/`emit` e anche il formato su disco (dentro la
//! busta di `store`). Nei bindings hanno il prefisso `Hub` (`HubThread`...),
//! per non confondersi con i tipi generici del frontend.
//!
//! Compatibilita': i campi facoltativi hanno `#[serde(default)]`, cosi' un file
//! scritto prima che un campo esistesse resta leggibile. Aggiungere una variante
//! a un enum e' compatibile in lettura; rinominarla richiede una migrazione.

use super::channel::Channel;
use crate::board::{TaskAction, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

mod legacy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubThreadState")]
pub enum ThreadState {
    Open,
    Closed,
}

/// Chi ha scritto un messaggio. `bot` e' un segretario (`name` = il suo handle);
/// `external` e' un mittente fuori dall'app arrivato da una fonte (un'email):
/// `connector` e `instance` sono gli id del connettore e dell'istanza che lo
/// hanno consegnato, `name` come si presenta il mittente (ripulito, 1..=120
/// caratteri). Il suo testo e' un dato non fidato, mai una richiesta dell'utente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, rename = "HubAuthor")]
pub enum Author {
    User,
    System,
    Bot { name: String },
    External { connector: String, instance: String, name: String },
}

/// Da dove viene un messaggio arrivato da fuori dell'app: un elemento di una
/// fonte (autore `external`) o un messaggio dell'utente scritto in un canale di
/// contatto (autore `user`). `external_id` e' l'id dell'elemento nel servizio
/// esterno (1..=256 caratteri, senza caratteri di controllo): con `instance`
/// identifica l'elemento per il dedupe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubMessageOrigin")]
pub struct MessageOrigin {
    pub connector: String,
    pub instance: String,
    pub external_id: String,
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
    /// Il worker (o il pianificatore) ha fatto una domanda all'utente
    /// (`ask_user`): `question` porta id, testo e opzioni.
    TaskQuestion,
    /// La domanda in attesa del task non aspetta piu': ha avuto una risposta
    /// (dal thread o direttamente nel terminale del worker).
    TaskQuestionAnswered,
    ProposalCreated,
    ProposalAccepted,
    ProposalRejected,
    /// Un segretario chiede di eseguire un'azione irreversibile su un task: la
    /// scheda con «Conferma» e «Annulla» (`request`). La esegue solo l'utente.
    ActionRequest,
    /// L'esito di una richiesta (confermata, annullata, scaduta, non piu'
    /// valida, fallita), in risposta alla scheda: sveglia chi l'ha chiesta.
    ActionRequestResolved,
}

/// Di cosa parla un evento: l'attesa dell'utente si calcola per soggetto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subject<'a> {
    Task(&'a str),
    Proposal(&'a str),
    /// Una richiesta di azione (l'id del suo messaggio): non e' mai il task,
    /// cosi' non interferisce con le attese e le domande del task.
    Request(&'a str),
}

impl HubEventKind {
    /// Lo stato che l'evento lascia aspetta una mossa dell'utente. Una
    /// richiesta di azione no: non e' una cosa che il thread deve all'utente.
    pub fn waits_for_user(self) -> bool {
        matches!(self, Self::TaskFailed | Self::TaskAttention | Self::TaskQuestion | Self::ProposalCreated)
    }

    fn is_about_proposal(self) -> bool {
        matches!(self, Self::ProposalCreated | Self::ProposalAccepted | Self::ProposalRejected)
    }

    fn is_about_request(self) -> bool {
        matches!(self, Self::ActionRequest | Self::ActionRequestResolved)
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
    /// Riassunto breve (per esempio dal report del worker), accorciato con "…"
    /// se il testo intero e' piu' lungo (vedi `summary_chars`).
    #[serde(default)]
    pub summary: Option<String>,
    /// Lunghezza in caratteri del riassunto intero, solo se `summary` e'
    /// accorciato: il testo intero e' nel report del task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub summary_chars: Option<usize>,
    /// Esito di una revisione completata.
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// Nota dell'app sul task (per esempio il motivo di un fallimento),
    /// accorciata con "…" se il testo intero e' piu' lungo (vedi `note_chars`).
    #[serde(default)]
    pub note: Option<String>,
    /// Lunghezza in caratteri della nota intera, solo se `note` e' accorciata:
    /// il testo intero e' nella nota del task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note_chars: Option<usize>,
    /// La domanda del worker, negli eventi `task_question`.
    #[serde(default)]
    pub question: Option<HubQuestion>,
    /// La richiesta di azione, negli eventi `action_request` e
    /// `action_request_resolved` (in questi ultimi con lo stato finale).
    #[serde(default)]
    pub request: Option<HubActionRequest>,
}

/// Lo stato di una richiesta di azione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubActionRequestState")]
pub enum ActionRequestState {
    /// Aspetta il clic dell'utente.
    Pending,
    /// L'utente ha confermato e l'azione e' stata eseguita.
    Confirmed,
    Cancelled,
    /// Scaduta senza una decisione: nessuna azione.
    Expired,
    /// Il task e' cambiato (stato, sparito, azione non piu' ammessa): nessuna azione.
    Stale,
    /// L'utente ha confermato ma la board ha rifiutato l'azione.
    Failed,
}

/// Cosa chiede una richiesta: un'azione sul task che non si annulla (i nomi sul
/// filo sono quelli di `board::TaskAction`) o la modifica dei suoi campi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubRequestedAction")]
pub enum RequestedAction {
    Stop,
    Delete,
    DeleteCascade,
    MarkDone,
    /// Modifica titolo, specifica, dipendenze o modello (`HubActionRequest::update`).
    Update,
}

impl RequestedAction {
    /// L'azione della board che la richiesta esegue; `None` per una modifica.
    pub fn task_action(self) -> Option<TaskAction> {
        match self {
            Self::Stop => Some(TaskAction::Stop),
            Self::Delete => Some(TaskAction::Delete),
            Self::DeleteCascade => Some(TaskAction::DeleteCascade),
            Self::MarkDone => Some(TaskAction::MarkDone),
            Self::Update => None,
        }
    }
}

/// Un campo di un task che un segretario puo' chiedere di cambiare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubTaskField")]
pub enum TaskField {
    Title,
    Task,
    DependsOn,
    Model,
    /// L'id del Bot: con il Bot cambiano anche agente, modello, effort e skill del task.
    Bot,
}

/// Prima e dopo di un campo, per la scheda: i testi lunghi sono troncati (la
/// conferma applica quelli completi di `TaskUpdate::patch`). `None` = nessun
/// valore (modello predefinito, nessuna dipendenza, nessun Bot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubFieldChange")]
pub struct FieldChange {
    pub field: TaskField,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// La modifica di un task chiesta da un segretario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubTaskUpdate")]
pub struct TaskUpdate {
    /// Il diff mostrato nella scheda, un elemento per campo cambiato.
    pub changes: Vec<FieldChange>,
    /// I valori nuovi completi, come i campi di `board::TaskSpec` (il patch di `update_task`).
    #[ts(skip)]
    pub patch: Value,
    /// Gli stessi campi com'erano quando la modifica e' stata chiesta: se il
    /// task non li ha piu' cosi', la richiesta non vale.
    #[ts(skip)]
    pub base: Value,
}

/// Una richiesta di un segretario su un task che cambia cosa fara' l'agente o
/// non si annulla: si esegue solo dopo il clic dell'utente (vedi `engine::confirm`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubActionRequest")]
pub struct HubActionRequest {
    /// L'id del messaggio della scheda.
    pub id: String,
    pub action: RequestedAction,
    /// Il motivo, scritto dal segretario.
    pub reason: String,
    /// L'handle del segretario che l'ha chiesta.
    pub secretary: String,
    pub task_title: String,
    /// Lo stato del task quando e' stata chiesta: se cambia, la richiesta non vale piu'.
    pub task_status: String,
    /// I task che una `delete_cascade` toglierebbe insieme al task (id).
    #[serde(default)]
    pub dependents: Vec<String>,
    /// La modifica chiesta, se `action` e' `update`.
    #[serde(default)]
    pub update: Option<TaskUpdate>,
    /// Scadenza, in millisecondi dall'epoca.
    pub expires_at: i64,
    pub state: ActionRequestState,
    /// Quando e' stata decisa o e' decaduta.
    #[serde(default)]
    pub resolved_at: Option<i64>,
    /// Spiegazione, in italiano, di un esito che non e' una conferma.
    #[serde(default)]
    pub note: Option<String>,
}

/// La domanda di un worker all'utente riportata in un thread: l'id con cui le
/// si risponde (`board::TaskQuestion::id`), il testo e le risposte suggerite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubQuestion")]
pub struct HubQuestion {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
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
            summary_chars: None,
            verdict: None,
            note: None,
            note_chars: None,
            question: None,
            request: None,
        }
    }

    /// Il soggetto dell'evento: il task per gli eventi dei task, la proposta per
    /// quelli delle proposte, la richiesta per quelli delle richieste. `None`
    /// se manca l'id che il tipo richiede.
    pub fn subject(&self) -> Option<Subject<'_>> {
        if self.kind.is_about_request() {
            self.request.as_ref().map(|r| Subject::Request(&r.id))
        } else if self.kind.is_about_proposal() {
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
    /// Gli allegati di un messaggio dell'utente (solo i metadati: i file sono in
    /// `hub_attachments/`, vedi `attachments`). Vuoto negli altri messaggi e nei
    /// file scritti prima.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Da dove viene il messaggio, se e' arrivato da fuori dell'app (vedi
    /// `MessageOrigin`). Assente nei messaggi scritti nell'app e nei file
    /// scritti prima.
    #[serde(default)]
    pub origin: Option<MessageOrigin>,
    pub created_at: i64,
}

/// Un file allegato a un messaggio. Il percorso non si salva: si ricava da
/// cartella dati, canale, thread, `id` e `name` (`attachments::layout`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "HubAttachment")]
pub struct Attachment {
    /// `att-<millisecondi>-<n>`.
    pub id: String,
    /// Il nome originale ripulito per Windows (`attachments::name::clean`).
    pub name: String,
    /// Dedotto da contenuto ed estensione; `application/octet-stream` se ignoto.
    pub mime: String,
    /// In byte.
    #[ts(type = "number")]
    pub size: u64,
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
    /// Le proposte collegate ancora da decidere, dalla piu' vecchia: decisa,
    /// una proposta si toglie (vedi `events::route`). Una proposta appartiene
    /// a un solo thread. I file scritti prima hanno `proposal_id`, una sola.
    #[serde(default, alias = "proposal_id", deserialize_with = "legacy::proposal_ids")]
    pub proposal_ids: Vec<String>,
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
