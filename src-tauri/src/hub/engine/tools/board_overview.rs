//! `board_overview`: descrizione e schema per il modello (in inglese). Non ha
//! parametri: legge ogni workspace che il segretario puo' consultare.

use super::super::chat::ToolSpec;
use crate::hub::settings::SecretaryTool;
use serde_json::json;

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: "Overview of every workspace you can read, in one call: for each one its name, the number of \
            tasks per status, the tasks that need the user, the worker questions still unanswered and whether the \
            board is paused. Use it for \"where are we?\" questions, especially in the General channel; then use \
            `board_status` or `task_details` to look closer at one workspace or task.",
        parameters: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    }
}
