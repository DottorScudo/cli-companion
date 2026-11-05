//! `relay_answer`: descrizione e schema per il modello (in inglese) e
//! validazione degli argomenti.

use super::super::chat::ToolSpec;
use super::super::relay::QuestionSelector;
use super::arguments;
use super::task_details::TASK_ID_MAX;
use crate::board::ANSWER_MAX;
use crate::hub::settings::SecretaryTool;
use crate::hub::text;
use serde::Deserialize;
use serde_json::json;

/// Caratteri massimi dell'id di una domanda.
pub const QUESTION_ID_MAX: usize = 64;

const DESCRIPTION: &str = "Deliver an answer to a worker question that is waiting in this thread, typed into the \
    worker's terminal as the user's reply. Call it only when the user explicitly asks you to answer on their behalf \
    (for example \"tell it to use the second option\"), and write in `body` exactly what the user wants the worker \
    to know, with no additions. It works only when the user's message woke you up. With exactly one question \
    waiting, pass no id. With several, pass the `question_id` and the `task_id` of the one the user means, as shown \
    in the question events returned by `thread_read`; if the user did not say which, ask them, and tell them they \
    can also use Reply on that question message. An id that is not of a waiting question is refused.";

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    ToolSpec {
        name: tool.code(),
        description: DESCRIPTION,
        parameters: json!({
            "type": "object",
            "properties": {
                "body": {
                    "type": "string",
                    "description": "The answer for the worker, as the user meant it.",
                    "minLength": 1,
                    "maxLength": ANSWER_MAX
                },
                "question_id": {
                    "type": "string",
                    "description": "Id of the waiting question to answer (`event.question.id` of its event in \
                        `thread_read`). Omit it when only one question is waiting.",
                    "maxLength": QUESTION_ID_MAX
                },
                "task_id": {
                    "type": "string",
                    "description": "Id of the task that asked it (`event.task_id`): a question id is unique only \
                        within its task, so pass both.",
                    "maxLength": TASK_ID_MAX
                }
            },
            "required": ["body"],
            "additionalProperties": false
        }),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    body: String,
    #[serde(default)]
    question_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
}

/// Il testo della risposta e la domanda scelta; l'errore e' in inglese, per il
/// modello.
pub fn parse(raw: &str) -> Result<(String, QuestionSelector), String> {
    let args: Args = arguments(raw)?;
    let body_error = || format!("`body` must be 1 to {ANSWER_MAX} characters of plain text");
    let body = text::body(&args.body).map_err(|_| body_error())?;
    if body.chars().count() > ANSWER_MAX {
        return Err(body_error());
    }
    let selector = QuestionSelector {
        question_id: identifier(args.question_id, "question_id", QUESTION_ID_MAX)?,
        task_id: identifier(args.task_id, "task_id", TASK_ID_MAX)?,
    };
    Ok((body, selector))
}

/// Un id opzionale: vuoto vale assente, oltre `max` caratteri e' rifiutato.
fn identifier(raw: Option<String>, name: &str, max: usize) -> Result<Option<String>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let id = raw.trim();
    if id.chars().count() > max {
        return Err(format!("`{name}` must be at most {max} characters"));
    }
    Ok((!id.is_empty()).then(|| id.to_string()))
}
