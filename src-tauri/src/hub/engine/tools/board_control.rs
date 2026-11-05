//! `board_control`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti, con gli stessi limiti di `max_parallel` del
//! comando `board_set`.

use super::super::board_ops::BoardChange;
use super::super::chat::ToolSpec;
use super::super::workspace_names::REFERENCE_MAX;
use super::{arguments, workspace_reference};
use crate::board::{MAX_PARALLEL_LIMIT, MIN_PARALLEL_LIMIT};
use crate::hub::settings::SecretaryTool;
use serde::Deserialize;
use serde_json::json;

const DESCRIPTION: &str = "Pause or resume the queue of this thread's board, or set how many tasks it runs at the \
    same time, right away and without asking the user. A paused board starts no new task; running tasks go on. \
    Call it only when the user asks for that change, passing only what they want changed. It works only on the \
    board of this thread's channel: in the General channel pass `workspace`.";

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: DESCRIPTION,
        parameters: json!({
            "type": "object",
            "properties": {
                "paused": {
                    "type": "boolean",
                    "description": "true to pause the queue, false to resume it."
                },
                "max_parallel": {
                    "type": "integer",
                    "description": "How many tasks of this board may run at the same time.",
                    "minimum": MIN_PARALLEL_LIMIT,
                    "maximum": MAX_PARALLEL_LIMIT
                },
                "workspace": {
                    "type": "string",
                    "description": "Name of the workspace of the board (an id also works). Needed only in the \
                        General channel; elsewhere it can only be this thread's channel.",
                    "maxLength": REFERENCE_MAX
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
    paused: Option<bool>,
    #[serde(default)]
    max_parallel: Option<i64>,
    #[serde(default)]
    workspace: Option<String>,
}

/// Il workspace chiesto (`None` = il canale del thread) e la modifica, mai
/// vuota; l'errore e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<(Option<String>, BoardChange), String> {
    let args: Args = arguments(raw)?;
    let max_parallel = args.max_parallel.map(parallel).transpose()?;
    let change = BoardChange { paused: args.paused, max_parallel };
    if change.is_empty() {
        return Err("pass `paused`, `max_parallel` or both".to_string());
    }
    Ok((workspace_reference(args.workspace.as_deref())?, change))
}

fn parallel(raw: i64) -> Result<u8, String> {
    u8::try_from(raw)
        .ok()
        .filter(|n| (MIN_PARALLEL_LIMIT..=MAX_PARALLEL_LIMIT).contains(n))
        .ok_or_else(|| format!("`max_parallel` must be an integer from {MIN_PARALLEL_LIMIT} to {MAX_PARALLEL_LIMIT}"))
}
