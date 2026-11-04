//! Il client reale del modello: `async-openai` verso un endpoint compatibile
//! OpenAI (`base_url` + `model` del provider, API key facoltativa). Converte
//! i tipi del motore (`chat`) in quelli della libreria e classifica gli errori
//! senza mai riportare la chiave ne' il corpo delle risposte.

use super::chat::{ChatMessage, ChatModel, ChatRequest, ModelError, ModelReply, ToolCall, ToolSpec};
use crate::hub::settings::Provider;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestMessage, ChatCompletionRequestToolMessage, ChatCompletionTool, ChatCompletionTools,
    CreateChatCompletionRequest, CreateChatCompletionResponse, FunctionCall, FunctionObject,
};
use async_openai::Client;
use std::time::Duration;

/// Tempo massimo di una chiamata, risposta compresa: un modello locale su una
/// macchina lenta puo' metterci decine di secondi.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Prefisso degli id dati alle chiamate a tool che l'endpoint lascia senza id.
const CALL_ID_PREFIX: &str = "call";

pub struct OpenAiModel {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiModel {
    /// Client per il provider; `NotConfigured` se mancano indirizzo o modello.
    pub fn new(provider: &Provider, api_key: Option<String>) -> Result<Self, ModelError> {
        if !provider.is_configured() {
            return Err(ModelError::NotConfigured);
        }
        ensure_crypto_provider();
        let http =
            reqwest::Client::builder().timeout(REQUEST_TIMEOUT).connect_timeout(CONNECT_TIMEOUT).build().map_err(
                |e| {
                    log::error!("hub: client HTTP dei segretari non creato: {e}");
                    ModelError::Unreachable
                },
            )?;
        Ok(Self { client: Client::build(http, config(provider, api_key)), model: provider.model.clone() })
    }
}

impl ChatModel for OpenAiModel {
    async fn complete(&self, request: ChatRequest<'_>) -> Result<ModelReply, ModelError> {
        match self.client.chat().create(to_request(&self.model, request)).await {
            Ok(response) => from_response(response),
            Err(e) => Err(classify(&e)),
        }
    }
}

/// `OpenAIConfig::new` legge `OPENAI_*` dall'ambiente: ogni campo si
/// sovrascrive, cosi' nessun valore dell'ambiente (una chiave OpenAI
/// dell'utente) arriva all'endpoint scelto nelle impostazioni.
fn config(provider: &Provider, api_key: Option<String>) -> OpenAIConfig {
    OpenAIConfig::new()
        .with_api_base(provider.base_url.as_str())
        .with_api_key(api_key.unwrap_or_default())
        .with_org_id("")
        .with_project_id("")
}

/// reqwest e' compilato senza provider crittografico (come per l'updater):
/// si installa `ring` se nessuno l'ha ancora fatto. Se un'installazione
/// concorrente vince, il provider c'e' comunque: l'esito si puo' ignorare.
fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

fn to_request(model: &str, request: ChatRequest<'_>) -> CreateChatCompletionRequest {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(ChatCompletionRequestMessage::System(request.system.into()));
    messages.extend(request.messages.iter().map(to_message));
    let tools = (!request.tools.is_empty()).then(|| request.tools.iter().map(to_tool).collect());
    CreateChatCompletionRequest { messages, model: model.to_string(), tools, ..Default::default() }
}

fn to_message(message: &ChatMessage) -> ChatCompletionRequestMessage {
    match message {
        ChatMessage::User(text) => ChatCompletionRequestMessage::User(text.as_str().into()),
        ChatMessage::Assistant { text, tool_calls } => {
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: text.as_deref().map(Into::into),
                tool_calls: (!tool_calls.is_empty()).then(|| tool_calls.iter().map(to_tool_call).collect()),
                ..Default::default()
            })
        }
        ChatMessage::Tool { call_id, content } => {
            ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
                content: content.as_str().into(),
                tool_call_id: call_id.clone(),
            })
        }
    }
}

fn to_tool_call(call: &ToolCall) -> ChatCompletionMessageToolCalls {
    ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
        id: call.id.clone(),
        function: FunctionCall { name: call.name.clone(), arguments: call.arguments.clone() },
    })
}

fn to_tool(spec: &ToolSpec) -> ChatCompletionTools {
    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: spec.name.to_string(),
            description: Some(spec.description.to_string()),
            parameters: Some(spec.parameters.clone()),
            strict: None,
        },
    })
}

/// La prima scelta: le chiamate a tool se ci sono, altrimenti il testo (o il
/// rifiuto del modello, che e' comunque la sua risposta).
fn from_response(response: CreateChatCompletionResponse) -> Result<ModelReply, ModelError> {
    let choice = response.choices.into_iter().next().ok_or(ModelError::BadResponse)?;
    let message = choice.message;
    let text = message.content.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let calls: Vec<ToolCall> =
        message.tool_calls.unwrap_or_default().into_iter().enumerate().map(|(i, c)| tool_call(i, c)).collect();
    if !calls.is_empty() {
        return Ok(ModelReply::ToolCalls { text, calls });
    }
    text.or_else(|| message.refusal.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()))
        .map(ModelReply::Text)
        .ok_or(ModelError::Empty)
}

fn tool_call(index: usize, call: ChatCompletionMessageToolCalls) -> ToolCall {
    let (id, name, arguments) = match call {
        ChatCompletionMessageToolCalls::Function(c) => (c.id, c.function.name, c.function.arguments),
        ChatCompletionMessageToolCalls::Custom(c) => (c.id, c.custom_tool.name, c.custom_tool.input),
    };
    let id = if id.is_empty() { format!("{CALL_ID_PREFIX}_{index}") } else { id };
    ToolCall { id, name, arguments }
}

/// Classifica l'errore e lo registra nel log senza il corpo della risposta
/// (un 401 di alcuni provider ripete parte della chiave).
fn classify(error: &OpenAIError) -> ModelError {
    let kind = match error {
        OpenAIError::Reqwest(e) if e.is_timeout() => ModelError::Timeout,
        OpenAIError::Reqwest(e) => match e.status() {
            Some(status) => ModelError::Http(status.as_u16()),
            None if e.is_decode() => ModelError::BadResponse,
            None => ModelError::Unreachable,
        },
        OpenAIError::ApiError(response) => ModelError::Http(response.status_code.as_u16()),
        _ => ModelError::BadResponse,
    };
    match error {
        OpenAIError::Reqwest(e) => log::warn!("hub: chiamata al modello fallita ({kind:?}): {e}"),
        _ => log::warn!("hub: chiamata al modello fallita ({kind:?})"),
    }
    kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::config::Config;
    use async_openai::error::{ApiError, ApiErrorResponse};
    use serde_json::json;

    fn provider() -> Provider {
        Provider::new("http://127.0.0.1:8080/v1", "qwen").unwrap()
    }

    fn history() -> Vec<ChatMessage> {
        vec![
            ChatMessage::User("status?".into()),
            ChatMessage::Assistant {
                text: None,
                tool_calls: vec![ToolCall { id: "c1".into(), name: "thread_read".into(), arguments: "{}".into() }],
            },
            ChatMessage::Tool { call_id: "c1".into(), content: "{\"title\":\"x\"}".into() },
        ]
    }

    fn tools() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "thread_read",
            description: "Read the thread.",
            parameters: json!({"type":"object","properties":{}}),
        }]
    }

    #[test]
    fn requests_carry_system_history_and_tools() {
        let (messages, tools) = (history(), tools());
        let request = ChatRequest { system: "You are mario.", messages: &messages, tools: &tools };
        let wire = serde_json::to_value(to_request("qwen", request)).unwrap();
        assert_eq!(wire["model"], "qwen");
        let messages = wire["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(
            (messages[0]["role"].as_str(), messages[0]["content"].as_str()),
            (Some("system"), Some("You are mario."))
        );
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["tool_calls"][0]["function"]["name"], "thread_read");
        assert_eq!(messages[2]["tool_calls"][0]["type"], "function");
        assert!(messages[2].get("content").is_none());
        assert_eq!((messages[3]["role"].as_str(), messages[3]["tool_call_id"].as_str()), (Some("tool"), Some("c1")));
        assert_eq!(wire["tools"][0]["function"]["parameters"]["type"], "object");
        assert!(wire.get("stream").is_none());
    }

    #[test]
    fn requests_without_tools_offer_none() {
        let messages = history();
        let request = ChatRequest { system: "s", messages: &messages, tools: &[] };
        assert!(serde_json::to_value(to_request("m", request)).unwrap().get("tools").is_none());
    }

    fn response(message: serde_json::Value) -> CreateChatCompletionResponse {
        serde_json::from_value(json!({
            "id": "x", "object": "chat.completion", "created": 1, "model": "qwen",
            "choices": [{ "index": 0, "message": message, "finish_reason": "stop" }]
        }))
        .unwrap()
    }

    #[test]
    fn text_replies_are_trimmed() {
        let reply = from_response(response(json!({"role":"assistant","content":"  Tutto ok.  "}))).unwrap();
        assert_eq!(reply, ModelReply::Text("Tutto ok.".into()));
    }

    #[test]
    fn tool_calls_win_over_text_and_get_an_id_when_missing() {
        let reply = from_response(response(json!({
            "role": "assistant", "content": "checking",
            "tool_calls": [
                {"id": "", "type": "function", "function": {"name": "board_status", "arguments": "{}"}},
                {"id": "c2", "type": "function", "function": {"name": "thread_read", "arguments": ""}}
            ]
        })))
        .unwrap();
        let ModelReply::ToolCalls { text, calls } = reply else { panic!("{reply:?}") };
        assert_eq!(text.as_deref(), Some("checking"));
        assert_eq!((calls[0].id.as_str(), calls[0].name.as_str()), ("call_0", "board_status"));
        assert_eq!(calls[1].id, "c2");
    }

    #[test]
    fn empty_or_missing_choices_are_errors() {
        assert_eq!(from_response(response(json!({"role":"assistant","content":"  "}))), Err(ModelError::Empty));
        assert_eq!(
            from_response(response(json!({"role":"assistant","refusal":"No."}))),
            Ok(ModelReply::Text("No.".into()))
        );
        let mut none = response(json!({"role":"assistant","content":"x"}));
        none.choices.clear();
        assert_eq!(from_response(none), Err(ModelError::BadResponse));
    }

    #[test]
    fn api_and_format_errors_are_classified() {
        let api = OpenAIError::ApiError(ApiErrorResponse {
            status_code: reqwest::StatusCode::UNAUTHORIZED,
            api_error: ApiError { message: "Incorrect API key provided: sk-12***".into(), ..Default::default() },
        });
        assert_eq!(classify(&api), ModelError::Http(401));
        let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        assert_eq!(classify(&OpenAIError::JSONDeserialize(json, "{".into())), ModelError::BadResponse);
    }

    #[test]
    fn the_client_needs_a_configured_provider() {
        assert!(matches!(OpenAiModel::new(&Provider::default(), None), Err(ModelError::NotConfigured)));
        assert!(OpenAiModel::new(&provider(), Some("k".into())).is_ok());
    }

    #[test]
    fn the_config_sends_only_the_chosen_key_to_the_chosen_endpoint() {
        let with_key = config(&provider(), Some("secret".into()));
        assert_eq!(with_key.url("/chat/completions"), "http://127.0.0.1:8080/v1/chat/completions");
        let headers = with_key.headers();
        assert_eq!(headers.get("authorization").unwrap(), "Bearer secret");
        assert!(headers.get("OpenAI-Organization").is_none() && headers.get("OpenAI-Project").is_none());
        let without = config(&provider(), None);
        assert_eq!(without.headers().get("authorization").unwrap(), "Bearer ");
    }
}
