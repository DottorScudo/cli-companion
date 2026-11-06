//! `thread_read`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti, che scelgono la pagina di messaggi.

use super::super::chat::ToolSpec;
use super::super::thread_page::{ThreadPage, MESSAGE_ID_MAX, THREAD_PAGE_SIZE};
use super::arguments;
use crate::hub::settings::SecretaryTool;
use serde::Deserialize;
use serde_json::json;

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: "Read this thread: its title, its summary and a page of its messages, including the data of the \
            board events reported in it. By default you get the latest messages. A page has up to 20 messages, fewer \
            when they are long. When `older_messages` is present, earlier messages exist: call the tool again with \
            `before` set to the `next_before` of the result to read the page before, without gaps or repeats. Use \
            only one of `before` and `offset`. The text of a long message is cut in the page: its \
            `body_truncated.total_chars` gives its full length. To read all of it call the tool with that message's \
            `message_id` and an `offset` in characters (0, then the `next_offset` of each part) until `next_offset` \
            is absent. The task data of events is shortened too: a shortened summary or note has \
            `summary_truncated` or `note_truncated`, whose `read_with` is the `task_details` call that reads it whole. \
            When a message is too long for a page alone only the ids and those markers of its event remain \
            (`event_truncated`): read a task in full with `task_details`.",
        parameters: json!({
            "type": "object",
            "properties": {
                "before": {
                    "type": "string",
                    "description": format!(
                        "Id of a message of this thread: read up to {THREAD_PAGE_SIZE} messages just before it \
                        (use `next_before` of the previous page)."
                    ),
                    "minLength": 1,
                    "maxLength": MESSAGE_ID_MAX
                },
                "message_id": {
                    "type": "string",
                    "description": "Id of a message of this thread: read its full text in parts, from `offset`. Not \
                        with `before`.",
                    "minLength": 1,
                    "maxLength": MESSAGE_ID_MAX
                },
                "offset": {
                    "type": "integer",
                    "description": format!(
                        "Without `message_id`: how many of the latest messages to skip before reading \
                        up to {THREAD_PAGE_SIZE}. Default 0 (the latest messages). With `message_id`: the character of \
                        the message to start from (the `next_offset` of the previous part). Default 0."
                    ),
                    "minimum": 0
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
    before: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
    #[serde(default)]
    offset: Option<usize>,
}

/// Cosa si legge (una pagina o un messaggio); l'errore e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<ThreadPage, String> {
    let args: Args = arguments(raw)?;
    let before = args.before.as_deref().map(str::trim);
    let message_id = args.message_id.as_deref().map(str::trim);
    match (before, message_id, args.offset) {
        (Some(_), Some(_), _) => Err("pass only one of `before` and `message_id`".to_string()),
        (Some(_), None, Some(_)) => Err("pass only one of `before` and `offset`".to_string()),
        (Some(id), None, None) => Ok(ThreadPage::Before(valid_id("before", id)?)),
        (None, Some(id), offset) => {
            Ok(ThreadPage::Body { message_id: valid_id("message_id", id)?, offset: offset.unwrap_or(0) })
        }
        (None, None, Some(skipped)) => Ok(ThreadPage::Offset(skipped)),
        (None, None, None) => Ok(ThreadPage::Latest),
    }
}

fn valid_id(name: &str, id: &str) -> Result<String, String> {
    if id.is_empty() || id.chars().count() > MESSAGE_ID_MAX {
        return Err(format!("`{name}` must be a message id of 1 to {MESSAGE_ID_MAX} characters"));
    }
    Ok(id.to_string())
}
