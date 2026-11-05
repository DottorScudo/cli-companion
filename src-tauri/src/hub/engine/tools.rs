//! I tool dei segretari: per ognuno lo schema JSON dei parametri e la
//! descrizione (in inglese: li legge il modello), scritti accanto alla struttura
//! degli argomenti in cui si validano. Il limite vero di cosa fa un segretario
//! sta qui: si accetta solo un tool della sua lista, con argomenti che
//! rispettano lo schema (campi sconosciuti rifiutati) e i limiti dell'HUB.
//! L'esecuzione e' in `turn` (attraverso `TurnHost`).

use super::audit::MAX_MUTATIONS_PER_TURN;
use super::board_ops::{BoardChange, TaskOp};
use super::board_query::BoardQuery;
use super::chat::{ToolCall, ToolSpec};
use super::planning::GOAL_MAX;
use super::relay::QuestionSelector;
use super::thread_page::ThreadPage;
use super::workspace_names::REFERENCE_MAX;
use crate::hub::channel::Channel;
use crate::hub::model::RequestedAction;
use crate::hub::settings::{Secretary, SecretaryTool};
use crate::hub::text::{self, clean_text, BODY_MAX};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// Lunghezza tenuta del nome di un tool sconosciuto, nei messaggi.
const UNKNOWN_NAME_KEPT: usize = 64;

/// Un tool validato, pronto da eseguire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    BoardStatus {
        /// `None` = la board del canale del thread.
        workspace: Option<String>,
        query: BoardQuery,
    },
    BoardOverview,
    TaskDetails {
        /// `None` = la board del canale del thread.
        workspace: Option<String>,
        task_id: String,
    },
    ThreadRead {
        page: ThreadPage,
    },
    PostMessage {
        body: String,
    },
    OpenPlanTask {
        goal: String,
    },
    DecideProposal {
        accept: bool,
    },
    RelayAnswer {
        body: String,
        /// Quale domanda in attesa; vuoto = l'unica.
        selector: QuestionSelector,
    },
    TaskAction {
        /// `None` = la board del canale del thread.
        workspace: Option<String>,
        task_id: String,
        op: TaskOp,
    },
    BoardControl {
        /// `None` = la board del canale del thread.
        workspace: Option<String>,
        change: BoardChange,
    },
    RequestTaskAction {
        task_id: String,
        action: RequestedAction,
        reason: String,
    },
    RequestTaskUpdate {
        task_id: String,
        /// I campi da cambiare, come il patch di `update_task`.
        patch: Value,
        reason: String,
    },
}

impl Invocation {
    /// Il tool chiamato.
    pub fn tool(&self) -> SecretaryTool {
        match self {
            Self::BoardStatus { .. } => SecretaryTool::BoardStatus,
            Self::BoardOverview => SecretaryTool::BoardOverview,
            Self::TaskDetails { .. } => SecretaryTool::TaskDetails,
            Self::ThreadRead { .. } => SecretaryTool::ThreadRead,
            Self::PostMessage { .. } => SecretaryTool::PostMessage,
            Self::OpenPlanTask { .. } => SecretaryTool::OpenPlanTask,
            Self::DecideProposal { .. } => SecretaryTool::DecideProposal,
            Self::RelayAnswer { .. } => SecretaryTool::RelayAnswer,
            Self::TaskAction { .. } => SecretaryTool::TaskAction,
            Self::BoardControl { .. } => SecretaryTool::BoardControl,
            Self::RequestTaskAction { .. } => SecretaryTool::RequestTaskAction,
            Self::RequestTaskUpdate { .. } => SecretaryTool::RequestTaskUpdate,
        }
    }
}

/// Perche' una chiamata a tool e' stata rifiutata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRejection {
    /// Il tool non esiste o non e' nella lista del segretario (nome ripulito).
    NotAllowed(String),
    /// Argomenti che non rispettano lo schema: `reason` in inglese, per il modello.
    InvalidArguments { tool: SecretaryTool, reason: String },
    /// Il turno ha gia' fatto `MAX_MUTATIONS_PER_TURN` modifiche (`audit::MutationBudget`).
    TurnLimit(SecretaryTool),
}

impl ToolRejection {
    /// L'esito restituito al modello (in inglese).
    pub fn for_model(&self) -> String {
        match self {
            Self::NotAllowed(name) => {
                format!("Tool `{name}` is not available to you. Use only the tools you were given.")
            }
            Self::InvalidArguments { tool, reason } => {
                format!("Invalid arguments for `{}`: {reason}. Check the parameters and try again.", tool.code())
            }
            Self::TurnLimit(tool) => format!(
                "`{}` was not run: you already made {MAX_MUTATIONS_PER_TURN} changes in this turn, the most allowed. \
                Do not try again: tell the user what you did and that they can ask you for the rest.",
                tool.code()
            ),
        }
    }
}

/// Le specifiche dei tool del segretario, nell'ordine canonico.
pub fn specs(secretary: &Secretary) -> Vec<ToolSpec> {
    SecretaryTool::ALL.into_iter().filter(|t| secretary.may_use(*t)).map(spec).collect()
}

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    match tool {
        SecretaryTool::BoardStatus => board_status::spec(tool),
        SecretaryTool::BoardOverview => board_overview::spec(tool),
        SecretaryTool::TaskDetails => task_details::spec(tool),
        SecretaryTool::ThreadRead => thread_read::spec(tool),
        SecretaryTool::PostMessage => ToolSpec {
            name: tool.code(),
            description: "Post an intermediate message in this thread, for example to say what you are checking. \
                Your final answer is posted anyway: do not repeat it here.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "body": {
                        "type": "string",
                        "description": "Text of the message.",
                        "minLength": 1,
                        "maxLength": BODY_MAX
                    }
                },
                "required": ["body"],
                "additionalProperties": false
            }),
        },
        SecretaryTool::OpenPlanTask => ToolSpec {
            name: tool.code(),
            description: "Open a planning task in this channel's workspace: a coding agent reads the code, writes \
                the specifications and comes back in this thread with a proposal of tasks, or with questions for the \
                user. Use it when the user asks for new work (a feature, a fix, a change), not for status questions. \
                You become the owner of this thread: the proposal, the questions of the planner and of the workers, failed tasks, tasks that \
                need attention and the end of all tasks will wake you up. It fails in the General channel: ask the \
                user which project the work is for. If a previous plan ended with questions, put the original goal, \
                the questions and the user's answers in `goal`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "goal": {
                        "type": "string",
                        "description": "What the user wants, in English, complete and self-contained. The planner \
                            also receives the user's message verbatim, the thread summary and the latest messages.",
                        "minLength": 1,
                        "maxLength": GOAL_MAX
                    }
                },
                "required": ["goal"],
                "additionalProperties": false
            }),
        },
        SecretaryTool::DecideProposal => ToolSpec {
            name: tool.code(),
            description: "Accept or reject the pending task proposal that the user's message replies to. Call it \
                only when the user clearly says what to do with it (for example \"yes, accept\" or \"no, drop it\"); \
                if the intent is not clear, ask the user instead. There is no id: it works only when the message \
                that woke you up is the user's reply to the proposal event.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "accept": {
                        "type": "boolean",
                        "description": "true to accept the proposal and queue its tasks, false to reject it."
                    }
                },
                "required": ["accept"],
                "additionalProperties": false
            }),
        },
        SecretaryTool::RelayAnswer => relay_answer::spec(tool),
        SecretaryTool::TaskAction => task_action::spec(tool),
        SecretaryTool::BoardControl => board_control::spec(tool),
        SecretaryTool::RequestTaskAction => request_task_action::spec(tool),
        SecretaryTool::RequestTaskUpdate => request_task_update::spec(tool),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardOverviewArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostMessageArgs {
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenPlanTaskArgs {
    goal: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideProposalArgs {
    accept: bool,
}

/// Valida la chiamata: il tool deve essere nella lista del segretario e gli
/// argomenti devono rispettare il suo schema.
pub fn parse(call: &ToolCall, secretary: &Secretary) -> Result<Invocation, ToolRejection> {
    let tool = SecretaryTool::ALL
        .into_iter()
        .find(|t| t.code() == call.name && secretary.may_use(*t))
        .ok_or_else(|| ToolRejection::NotAllowed(text::clean_line(&call.name, UNKNOWN_NAME_KEPT)))?;
    let invalid = |reason: String| ToolRejection::InvalidArguments { tool, reason };
    match tool {
        SecretaryTool::BoardStatus => {
            let (workspace, query) = board_status::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::BoardStatus { workspace, query })
        }
        SecretaryTool::BoardOverview => {
            let _: BoardOverviewArgs = arguments(&call.arguments).map_err(invalid)?;
            Ok(Invocation::BoardOverview)
        }
        SecretaryTool::TaskDetails => {
            let (workspace, task_id) = task_details::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::TaskDetails { workspace, task_id })
        }
        SecretaryTool::ThreadRead => {
            let page = thread_read::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::ThreadRead { page })
        }
        SecretaryTool::PostMessage => {
            let args: PostMessageArgs = arguments(&call.arguments).map_err(invalid)?;
            let body = text::body(&args.body)
                .map_err(|_| invalid(format!("`body` must be 1 to {BODY_MAX} characters of plain text")))?;
            Ok(Invocation::PostMessage { body })
        }
        SecretaryTool::OpenPlanTask => {
            let args: OpenPlanTaskArgs = arguments(&call.arguments).map_err(invalid)?;
            let goal = clean_text(args.goal.trim(), GOAL_MAX + 1);
            if goal.is_empty() || goal.chars().count() > GOAL_MAX {
                return Err(invalid(format!("`goal` must be 1 to {GOAL_MAX} characters of plain text")));
            }
            Ok(Invocation::OpenPlanTask { goal })
        }
        SecretaryTool::DecideProposal => {
            let args: DecideProposalArgs = arguments(&call.arguments).map_err(invalid)?;
            Ok(Invocation::DecideProposal { accept: args.accept })
        }
        SecretaryTool::RelayAnswer => {
            let (body, selector) = relay_answer::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::RelayAnswer { body, selector })
        }
        SecretaryTool::TaskAction => {
            let (workspace, task_id, op) = task_action::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::TaskAction { workspace, task_id, op })
        }
        SecretaryTool::BoardControl => {
            let (workspace, change) = board_control::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::BoardControl { workspace, change })
        }
        SecretaryTool::RequestTaskAction => {
            let (task_id, action, reason) = request_task_action::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::RequestTaskAction { task_id, action, reason })
        }
        SecretaryTool::RequestTaskUpdate => {
            let (task_id, patch, reason) = request_task_update::parse(&call.arguments).map_err(invalid)?;
            Ok(Invocation::RequestTaskUpdate { task_id, patch, reason })
        }
    }
}

/// Perche' il segretario non puo' leggere o cambiare la board chiesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardAccessError {
    /// Thread del Generale e nessun `workspace`: il Generale non ha una board.
    NoChannelBoard,
    /// Un workspace fuori dai canali del segretario.
    NotAllowed(String),
    /// Una modifica alla board di un workspace che non e' il canale del thread.
    OutsideThread(String),
}

/// Il workspace di cui `board_status` legge la board: quello del canale del
/// thread, o quello chiesto se il segretario ha quel canale.
pub fn board_workspace(
    requested: Option<String>,
    channel: &Channel,
    secretary: &Secretary,
) -> Result<String, BoardAccessError> {
    match (requested, channel) {
        (None, Channel::Workspace(id)) => Ok(id.clone()),
        (None, Channel::General) => Err(BoardAccessError::NoChannelBoard),
        (Some(id), _) if secretary.channels.allows(&Channel::Workspace(id.clone())) => Ok(id),
        (Some(id), _) => Err(BoardAccessError::NotAllowed(id)),
    }
}

/// Il workspace di cui `task_action` e `board_control` cambiano la board: solo
/// quello del canale del thread; nel Generale quello chiesto, se il segretario
/// ha quel canale.
pub fn change_workspace(
    requested: Option<String>,
    channel: &Channel,
    secretary: &Secretary,
) -> Result<String, BoardAccessError> {
    match (requested, channel) {
        (Some(id), Channel::Workspace(own)) if id != *own => Err(BoardAccessError::OutsideThread(id)),
        (requested, _) => board_workspace(requested, channel, secretary),
    }
}

/// Il riferimento a un workspace in un tool, ripulito (`None` = quello del
/// canale del thread); l'errore e' in inglese, per il modello.
fn workspace_reference(raw: Option<&str>) -> Result<Option<String>, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(reference) if reference.chars().count() > REFERENCE_MAX => {
            Err(format!("`workspace` must be at most {REFERENCE_MAX} characters"))
        }
        Some(reference) => Ok(Some(reference.to_string())),
    }
}

/// Argomenti vuoti valgono `{}` (alcuni endpoint mandano "" per un tool senza
/// parametri); il resto deve essere un oggetto JSON della forma attesa.
fn arguments<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    let raw = raw.trim();
    let value: Value = if raw.is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_str(raw).map_err(|_| "arguments are not valid JSON".to_string())?
    };
    if !value.is_object() {
        return Err("arguments must be a JSON object".to_string());
    }
    serde_json::from_value(value).map_err(|e| format!("arguments do not match the schema ({e})"))
}

mod board_control;
mod board_overview;
mod board_status;
pub(super) mod relay_answer;
mod request_task_action;
mod request_task_update;
mod task_action;
pub(super) mod task_details;
mod thread_read;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;
