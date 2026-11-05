//! `board_status`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti, che producono la ricerca (`BoardQuery`).

use super::super::board_query::{BoardQuery, PAGE_DEFAULT, PAGE_MAX, QUERY_MAX};
use super::super::chat::ToolSpec;
use super::super::workspace_names::REFERENCE_MAX;
use super::{arguments, workspace_reference};
use crate::board::{TaskKind, TaskStatus};
use crate::hub::settings::SecretaryTool;
use chrono::{DateTime, NaiveDate, NaiveTime};
use serde::Deserialize;
use serde_json::json;

/// Caratteri massimi di un istante ISO 8601 (data, ora con frazione e scarto).
const INSTANT_MAX: usize = 40;

const DESCRIPTION: &str = "Read the task board of a workspace (given by its name). Tasks are returned newest \
    first, a page at a time: `total` is how many tasks match, and when `next_offset` is present, call the tool again \
    with that `offset` to read the next page. Each task has its id, title, status, whether it needs the user's \
    attention, the tasks it waits on and a short summary of its last report; `counts` always covers the whole \
    board. Narrow the result with `status`, `attention` and `kind`, or search the id, title, report summary, note \
    and specification text with `query`, or select the tasks last updated in a period with `updated_since` and \
    `updated_before`: prefer filters and search to paging through every page. Without \
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
                "updated_since": {
                    "type": "string",
                    "description": "Only tasks updated at or after this instant: an ISO 8601 date (`2026-11-05`, \
                        midnight UTC) or date and time with offset (`2026-11-05T09:30:00Z`).",
                    "maxLength": INSTANT_MAX
                },
                "updated_before": {
                    "type": "string",
                    "description": "Only tasks updated before this instant, same format as `updated_since`. Use both \
                        to read a time window.",
                    "maxLength": INSTANT_MAX
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
    updated_since: Option<String>,
    #[serde(default)]
    updated_before: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    limit: Option<usize>,
}

/// Un istante ISO 8601 (`2026-11-05` = mezzanotte UTC, oppure data e ora con
/// scarto) in millisecondi UTC; l'errore e' in inglese, per il modello.
fn instant(field: &str, raw: &str) -> Result<i64, String> {
    let raw = raw.trim();
    let parsed = DateTime::parse_from_rfc3339(raw).map(|t| t.timestamp_millis()).ok().or_else(|| {
        let day = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
        Some(day.and_time(NaiveTime::MIN).and_utc().timestamp_millis())
    });
    parsed.ok_or_else(|| {
        format!("`{field}` must be an ISO 8601 date (2026-11-05) or date and time (2026-11-05T09:30:00Z)")
    })
}

/// Il workspace chiesto (`None` = il canale del thread) e la ricerca; l'errore
/// e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<(Option<String>, BoardQuery), String> {
    let args: Args = arguments(raw)?;
    let workspace = workspace_reference(args.workspace.as_deref())?;
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
    let updated_since = args.updated_since.as_deref().map(|raw| instant("updated_since", raw)).transpose()?;
    let updated_before = args.updated_before.as_deref().map(|raw| instant("updated_before", raw)).transpose()?;
    if updated_since.zip(updated_before).is_some_and(|(since, before)| since >= before) {
        return Err("`updated_since` must be earlier than `updated_before`".to_string());
    }
    let limit = args.limit.unwrap_or(PAGE_DEFAULT);
    if !(1..=PAGE_MAX).contains(&limit) {
        return Err(format!("`limit` must be between 1 and {PAGE_MAX}"));
    }
    let query = BoardQuery {
        statuses: args.status,
        attention: args.attention,
        kind: args.kind,
        text,
        updated_since,
        updated_before,
        offset: args.offset,
        limit,
    };
    Ok((workspace, query))
}
