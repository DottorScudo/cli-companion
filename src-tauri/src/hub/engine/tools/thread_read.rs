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
            board events reported in it. By default you get the latest messages. When `older_messages` is present, \
            earlier messages exist: call the tool again with `before` set to the `next_before` of the result (or with \
            an `offset`) to read the page before. Use only one of `before` and `offset`.",
        parameters: json!({
            "type": "object",
            "properties": {
                "before": {
                    "type": "string",
                    "description": format!(
                        "Id of a message of this thread: read the {THREAD_PAGE_SIZE} messages just before it \
                        (use `next_before` of the previous page)."
                    ),
                    "minLength": 1,
                    "maxLength": MESSAGE_ID_MAX
                },
                "offset": {
                    "type": "integer",
                    "description": format!(
                        "How many of the latest messages to skip before reading {THREAD_PAGE_SIZE}. Default 0 (the \
                        latest messages)."
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
    offset: Option<usize>,
}

/// La pagina chiesta; l'errore e' in inglese, per il modello.
pub fn parse(raw: &str) -> Result<ThreadPage, String> {
    let args: Args = arguments(raw)?;
    match (args.before.as_deref().map(str::trim), args.offset) {
        (Some(_), Some(_)) => Err("pass only one of `before` and `offset`".to_string()),
        (Some(id), None) if id.is_empty() || id.chars().count() > MESSAGE_ID_MAX => {
            Err(format!("`before` must be a message id of 1 to {MESSAGE_ID_MAX} characters"))
        }
        (Some(id), None) => Ok(ThreadPage::Before(id.to_string())),
        (None, Some(skipped)) => Ok(ThreadPage::Offset(skipped)),
        (None, None) => Ok(ThreadPage::Latest),
    }
}
