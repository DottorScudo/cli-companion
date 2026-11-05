//! `task_action`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti.

use super::super::board_ops::TaskOp;
use super::super::chat::ToolSpec;
use super::super::workspace_names::REFERENCE_MAX;
use super::task_details::TASK_ID_MAX;
use super::{arguments, workspace_reference};
use crate::hub::settings::SecretaryTool;
use serde::Deserialize;
use serde_json::json;

const DESCRIPTION: &str = "Change one task of this thread's board, right away and without asking the user: \
    `retry` puts a failed task back in the queue, `to_top` moves a queued task to the head of the queue, \
    `move_before` moves a queued task right before another queued task (`before_task_id`). Call it only when the \
    user asks for that change; use `task_details` first if you are not sure the task allows it (`allowed_actions`, \
    `can_reorder`). It works only on the board of this thread's channel: in the General channel pass `workspace`.";

/// Le azioni sul filo.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Retry,
    ToTop,
    MoveBefore,
}

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: DESCRIPTION,
        parameters: json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Id of the task to change, as shown by `board_status`.",
                    "minLength": 1,
                    "maxLength": TASK_ID_MAX
                },
                "action": {
                    "type": "string",
                    "enum": ["retry", "to_top", "move_before"],
                    "description": "What to do with the task."
                },
                "before_task_id": {
                    "type": "string",
                    "description": "Only with `move_before`: id of the queued task to place it before.",
                    "maxLength": TASK_ID_MAX
                },
                "workspace": {
                    "type": "string",
                    "description": "Name of the workspace of the task (an id also works). Needed only in the \
                        General channel; elsewhere it can only be this thread's channel.",
                    "maxLength": REFERENCE_MAX
                }
            },
            "required": ["task_id", "action"],
            "additionalProperties": false
        }),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    task_id: String,
    action: Action,
    #[serde(default)]
    before_task_id: Option<String>,
    #[serde(default)]
    workspace: Option<String>,
}

/// Il workspace chiesto (`None` = il canale del thread), l'id del task e
/// l'azione; l'errore e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<(Option<String>, String, TaskOp), String> {
    let args: Args = arguments(raw)?;
    let task_id = checked_id(&args.task_id, "task_id")?;
    let before = args.before_task_id.as_deref().map(str::trim).filter(|id| !id.is_empty());
    let op = match (args.action, before) {
        (Action::MoveBefore, Some(before)) => {
            TaskOp::MoveBefore { before_task_id: checked_id(before, "before_task_id")? }
        }
        (Action::MoveBefore, None) => return Err("`move_before` needs `before_task_id`".to_string()),
        (_, Some(_)) => return Err("`before_task_id` is only for `move_before`".to_string()),
        (Action::Retry, None) => TaskOp::Retry,
        (Action::ToTop, None) => TaskOp::ToTop,
    };
    Ok((workspace_reference(args.workspace.as_deref())?, task_id, op))
}

/// L'id di un task nel campo `field`, senza spazi ai bordi.
fn checked_id(raw: &str, field: &str) -> Result<String, String> {
    let id = raw.trim();
    if id.is_empty() || id.chars().count() > TASK_ID_MAX {
        return Err(format!("`{field}` must be 1 to {TASK_ID_MAX} characters"));
    }
    Ok(id.to_string())
}
