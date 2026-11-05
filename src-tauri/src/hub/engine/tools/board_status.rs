//! `board_status`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti, che producono la ricerca (`BoardQuery`).

use super::super::board_query::{BoardQuery, PAGE_DEFAULT, PAGE_MAX, QUERY_MAX};
use super::super::chat::ToolSpec;
use super::super::workspace_names::REFERENCE_MAX;
use super::arguments;
use crate::board::{TaskKind, TaskStatus};
use crate::hub::settings::SecretaryTool;
use serde::Deserialize;
use serde_json::json;

const DESCRIPTION: &str = "Read the task board of a workspace (given by its name). Tasks are returned newest \
    first, a page at a time: `total` is how many tasks match, and when `next_offset` is present, call the tool again \
    with that `offset` to read the next page. Each task has its id, title, status, whether it needs the user's \
    attention, the tasks it waits on and a short summary of its last report; `counts` always covers the whole \
    board. Narrow the result with `status`, `attention` and `kind`, or search the id, title, report summary, note \
    and specification text with `query`: prefer filters and search to paging through every page. Without \
    `workspace` it reads the board of this thread's channel.";

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: DESCRIPTION,
        parameters: json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Name of the workspace whose board to read (an id also works). Omit it to use \
                        this thread's channel.",
                    "maxLength": REFERENCE_MAX
                },
                "status": {
                    "type": "array",
                    "description": "Only tasks in one of these statuses. Omit it for every status.",
                    "items": { "type": "string", "enum": TaskStatus::ALL.map(TaskStatus::as_str) },
                    "uniqueItems": true,
                    "maxItems": TaskStatus::ALL.len()
                },
                "attention": {
                    "type": "boolean",
                    "description": "true = only tasks that need the user: the worker is waiting for them, it asked a \
                        question that is still unanswered, or the task cannot start because its agent is not ready."
                },
                "kind": {
                    "type": "string",
                    "description": "Only tasks of this kind: `work` delivers code, `review` checks the integrated \
                        result of other tasks, `plan` explores the code and proposes tasks.",
                    "enum": TaskKind::ALL.map(TaskKind::as_str)
                },
                "query": {
                    "type": "string",
                    "description": "Text to search, ignoring case, in the id, title, report summary, note and \
                        specification of each task. The specification itself is never returned.",
                    "maxLength": QUERY_MAX
                },
                "offset": {
                    "type": "integer",
                    "description": "How many matching tasks to skip (use `next_offset` of the previous page). \
                        Default 0.",
                    "minimum": 0
                },
                "limit": {
                    "type": "integer",
                    "description": format!("Tasks per page. Default {PAGE_DEFAULT}."),
                    "minimum": 1,
                    "maximum": PAGE_MAX
                }
            },
            "additionalProperties": false
        }),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    status: Vec<TaskStatus>,
    #[serde(default)]
    attention: bool,
    #[serde(default)]
    kind: Option<TaskKind>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    limit: Option<usize>,
}

/// Il workspace chiesto (`None` = il canale del thread) e la ricerca; l'errore
/// e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<(Option<String>, BoardQuery), String> {
    let args: Args = arguments(raw)?;
    let workspace = match args.workspace.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(reference) if reference.chars().count() > REFERENCE_MAX => {
            return Err(format!("`workspace` must be at most {REFERENCE_MAX} characters"));
        }
        Some(reference) => Some(reference.to_string()),
    };
    if let Some(repeated) = args.status.iter().enumerate().find(|(i, s)| args.status[..*i].contains(s)) {
        return Err(format!("`status` must not repeat a value (`{}` is repeated)", repeated.1.as_str()));
    }
    let text = match args.query.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(text) if text.chars().count() > QUERY_MAX => {
            return Err(format!("`query` must be at most {QUERY_MAX} characters"));
        }
        Some(text) => Some(text.to_string()),
    };
    let limit = args.limit.unwrap_or(PAGE_DEFAULT);
    if !(1..=PAGE_MAX).contains(&limit) {
        return Err(format!("`limit` must be between 1 and {PAGE_MAX}"));
    }
    let query = BoardQuery {
        statuses: args.status,
        attention: args.attention,
        kind: args.kind,
        text,
        offset: args.offset,
        limit,
    };
    Ok((workspace, query))
}
