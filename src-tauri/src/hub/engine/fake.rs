//! Il modello finto dei test: risponde con una sequenza scritta a mano (testo,
//! chiamate a tool o errori) e registra le richieste ricevute. Nessuna rete.

use super::chat::{ChatMessage, ChatModel, ChatRequest, ModelError, ModelReply, ToolCall};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Una richiesta ricevuta, copiata.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<&'static str>,
}

pub struct FakeModel {
    replies: Mutex<VecDeque<Result<ModelReply, ModelError>>>,
    requests: Mutex<Vec<Recorded>>,
}

impl FakeModel {
    pub fn new(replies: Vec<Result<ModelReply, ModelError>>) -> Self {
        Self { replies: Mutex::new(replies.into()), requests: Mutex::new(Vec::new()) }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

/// Risposta di solo testo.
pub fn text(body: &str) -> Result<ModelReply, ModelError> {
    Ok(ModelReply::Text(body.to_string()))
}

/// Una chiamata al tool `name` con gli argomenti JSON `arguments`.
pub fn call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall { id: id.to_string(), name: name.to_string(), arguments: arguments.to_string() }
}

/// Risposta con le chiamate a tool date.
pub fn calls(list: Vec<ToolCall>) -> Result<ModelReply, ModelError> {
    Ok(ModelReply::ToolCalls { text: None, calls: list })
}

impl ChatModel for FakeModel {
    async fn complete(&self, request: ChatRequest<'_>) -> Result<ModelReply, ModelError> {
        self.requests.lock().unwrap().push(Recorded {
            system: request.system.to_string(),
            messages: request.messages.to_vec(),
            tools: request.tools.iter().map(|t| t.name).collect(),
        });
        // Una sequenza esaurita e' un errore del test, non del motore.
        self.replies.lock().unwrap().pop_front().expect("risposta finta esaurita")
    }
}
