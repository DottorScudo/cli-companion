//! I tool dei segretari: per ognuno lo schema JSON dei parametri e la
//! descrizione (in inglese: li legge il modello), scritti accanto alla struttura
//! degli argomenti in cui si validano. Il limite vero di cosa fa un segretario
//! sta qui: si accetta solo un tool della sua lista, con argomenti che
//! rispettano lo schema (campi sconosciuti rifiutati) e i limiti dell'HUB.
//! L'esecuzione e' in `turn` (attraverso `TurnHost`).

use super::chat::{ToolCall, ToolSpec};
use crate::hub::channel::Channel;
use crate::hub::settings::{Secretary, SecretaryTool};
use crate::hub::text::{self, BODY_MAX};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// Lunghezza tenuta del nome di un tool sconosciuto, nei messaggi.
const UNKNOWN_NAME_KEPT: usize = 64;

/// Un tool validato, pronto da eseguire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// `None` = la board del canale del thread.
    BoardStatus {
        workspace: Option<String>,
    },
    ThreadRead,
    PostMessage {
        body: String,
    },
}

/// Perche' una chiamata a tool e' stata rifiutata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRejection {
    /// Il tool non esiste o non e' nella lista del segretario (nome ripulito).
    NotAllowed(String),
    /// Argomenti che non rispettano lo schema: `reason` in inglese, per il modello.
    InvalidArguments { tool: SecretaryTool, reason: String },
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
        }
    }
}

/// Le specifiche dei tool del segretario, nell'ordine canonico.
pub fn specs(secretary: &Secretary) -> Vec<ToolSpec> {
    SecretaryTool::ALL.into_iter().filter(|t| secretary.may_use(*t)).map(spec).collect()
}

pub fn spec(tool: SecretaryTool) -> ToolSpec {
    match tool {
        SecretaryTool::BoardStatus => ToolSpec {
            name: tool.code(),
            description: "Read the task board of a workspace: for each task its id, title, status, whether it \
                needs the user's attention, the tasks it waits on and a short summary of its last report. Without \
                `workspace` it reads the board of this thread's channel.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "description": "Id of the workspace whose board to read. Omit it to use this thread's channel."
                    }
                },
                "additionalProperties": false
            }),
        },
        SecretaryTool::ThreadRead => ToolSpec {
            name: tool.code(),
            description: "Read this thread: its title, its summary and its latest messages, including the data \
                of the board events reported in it.",
            parameters: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
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
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardStatusArgs {
    #[serde(default)]
    workspace: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThreadReadArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostMessageArgs {
    body: String,
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
            let args: BoardStatusArgs = arguments(&call.arguments).map_err(invalid)?;
            let workspace = match args.workspace.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(id) => match Channel::parse(id) {
                    Ok(Channel::Workspace(id)) => Some(id),
                    _ => return Err(invalid("`workspace` is not a valid workspace id".to_string())),
                },
            };
            Ok(Invocation::BoardStatus { workspace })
        }
        SecretaryTool::ThreadRead => {
            let _: ThreadReadArgs = arguments(&call.arguments).map_err(invalid)?;
            Ok(Invocation::ThreadRead)
        }
        SecretaryTool::PostMessage => {
            let args: PostMessageArgs = arguments(&call.arguments).map_err(invalid)?;
            let body = text::body(&args.body)
                .map_err(|_| invalid(format!("`body` must be 1 to {BODY_MAX} characters of plain text")))?;
            Ok(Invocation::PostMessage { body })
        }
    }
}

/// Perche' il segretario non puo' leggere la board chiesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardAccessError {
    /// Thread del Generale e nessun `workspace`: il Generale non ha una board.
    NoChannelBoard,
    /// Un workspace fuori dai canali del segretario.
    NotAllowed(String),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::settings::SecretaryChannels;

    fn secretary(tools: &[SecretaryTool]) -> Secretary {
        Secretary {
            name: "mario".into(),
            image: None,
            system_prompt: "Report.".into(),
            tools: tools.to_vec(),
            channels: SecretaryChannels::All,
        }
    }

    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall { id: "c".into(), name: name.into(), arguments: arguments.into() }
    }

    #[test]
    fn only_the_allowed_tools_are_offered() {
        let s = secretary(&[SecretaryTool::PostMessage, SecretaryTool::BoardStatus]);
        let names: Vec<&str> = specs(&s).iter().map(|t| t.name).collect();
        assert_eq!(names, ["board_status", "post_message"]);
        assert!(specs(&secretary(&[])).is_empty());
    }

    #[test]
    fn every_schema_is_a_closed_object_with_an_english_description() {
        for tool in SecretaryTool::ALL {
            let s = spec(tool);
            assert_eq!(s.name, tool.code());
            assert_eq!(s.parameters["type"], "object");
            assert_eq!(s.parameters["additionalProperties"], false);
            assert!(s.description.is_ascii() && !s.description.is_empty());
        }
    }

    #[test]
    fn schema_properties_match_the_argument_structs() {
        let all = secretary(&SecretaryTool::ALL);
        let samples = [
            ("board_status", json!({"workspace": "ws-1"})),
            ("thread_read", json!({})),
            ("post_message", json!({"body": "x"})),
        ];
        for (name, sample) in samples {
            let tool = SecretaryTool::ALL.into_iter().find(|t| t.code() == name).unwrap();
            let props = spec(tool).parameters["properties"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
            let keys = sample.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
            assert_eq!(props, keys, "{name}");
            assert!(parse(&call(name, &sample.to_string()), &all).is_ok(), "{name}");
        }
    }

    #[test]
    fn a_tool_outside_the_list_is_not_allowed() {
        let s = secretary(&[SecretaryTool::ThreadRead]);
        assert_eq!(parse(&call("board_status", "{}"), &s), Err(ToolRejection::NotAllowed("board_status".into())));
        assert_eq!(parse(&call("rm\n-rf", "{}"), &s), Err(ToolRejection::NotAllowed("rm -rf".into())));
        let long = "x".repeat(200);
        let Err(ToolRejection::NotAllowed(name)) = parse(&call(&long, "{}"), &s) else { panic!() };
        assert!(name.chars().count() <= UNKNOWN_NAME_KEPT + 1);
    }

    #[test]
    fn arguments_are_validated_against_the_schema() {
        let s = secretary(&SecretaryTool::ALL);
        assert_eq!(parse(&call("thread_read", ""), &s), Ok(Invocation::ThreadRead));
        assert_eq!(parse(&call("board_status", "{}"), &s), Ok(Invocation::BoardStatus { workspace: None }));
        assert_eq!(
            parse(&call("board_status", r#"{"workspace":" ws-2 "}"#), &s),
            Ok(Invocation::BoardStatus { workspace: Some("ws-2".into()) })
        );
        assert_eq!(
            parse(&call("post_message", r#"{"body":"  Controllo.  "}"#), &s),
            Ok(Invocation::PostMessage { body: "Controllo.".into() })
        );
        for (name, args) in [
            ("thread_read", r#"{"limit":3}"#),
            ("thread_read", "[]"),
            ("thread_read", "{"),
            ("board_status", r#"{"workspace":"general"}"#),
            ("board_status", r#"{"workspace":"../x"}"#),
            ("board_status", r#"{"workspace":3}"#),
            ("post_message", "{}"),
            ("post_message", r#"{"body":"   "}"#),
            ("post_message", r#"{"body":"a","extra":1}"#),
        ] {
            let err = parse(&call(name, args), &s).unwrap_err();
            assert!(matches!(err, ToolRejection::InvalidArguments { .. }), "{name} {args}");
            assert!(err.for_model().contains(name));
        }
        let long = json!({ "body": "a".repeat(BODY_MAX + 1) }).to_string();
        assert!(parse(&call("post_message", &long), &s).is_err());
    }

    #[test]
    fn the_board_is_the_channel_one_or_one_of_the_secretary_channels() {
        let ws = |id: &str| Channel::Workspace(id.into());
        let mut s = secretary(&[SecretaryTool::BoardStatus]);
        assert_eq!(board_workspace(None, &ws("ws-1"), &s), Ok("ws-1".into()));
        assert_eq!(board_workspace(None, &Channel::General, &s), Err(BoardAccessError::NoChannelBoard));
        assert_eq!(board_workspace(Some("ws-2".into()), &Channel::General, &s), Ok("ws-2".into()));
        s.channels = SecretaryChannels::Only { channels: vec![Channel::General, ws("ws-1")] };
        assert_eq!(board_workspace(Some("ws-1".into()), &Channel::General, &s), Ok("ws-1".into()));
        assert_eq!(
            board_workspace(Some("ws-2".into()), &ws("ws-1"), &s),
            Err(BoardAccessError::NotAllowed("ws-2".into()))
        );
    }
}
