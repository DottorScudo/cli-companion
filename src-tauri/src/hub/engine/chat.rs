//! La conversazione con il modello, nei tipi del motore: messaggi, chiamate a
//! tool, richiesta e risposta, e il trait `ChatModel` che le trasporta. Il
//! client reale (`openai`) e il finto dei test (`fake`) lo implementano; il
//! ciclo di tool calling (`turn`) non conosce il protocollo sul filo.

use serde_json::Value;
use std::future::Future;

/// Un messaggio della conversazione interna di una sessione. Il system prompt
/// non e' qui: si ricostruisce a ogni passo (`ChatRequest::system`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatMessage {
    User(String),
    Assistant { text: Option<String>, tool_calls: Vec<ToolCall> },
    Tool { call_id: String, content: String },
}

/// Un tool chiesto dal modello: `arguments` e' il JSON grezzo, ancora da validare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Un tool offerto al modello: nome, descrizione e schema JSON dei parametri.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

/// Una chiamata: prende in prestito la conversazione, che a ogni passo cresce
/// senza essere copiata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChatRequest<'a> {
    pub system: &'a str,
    pub messages: &'a [ChatMessage],
    /// Vuoto = il modello puo' solo rispondere con del testo.
    pub tools: &'a [ToolSpec],
}

/// Cosa ha risposto il modello: del testo o delle chiamate a tool (con
/// l'eventuale testo che le accompagna).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelReply {
    Text(String),
    ToolCalls { text: Option<String>, calls: Vec<ToolCall> },
}

/// Perche' una chiamata al modello e' fallita, senza dettagli sensibili: il
/// testo per l'utente e' in `engine::texts::model_error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// Indirizzo o modello del provider non impostati.
    NotConfigured,
    /// La API key non si legge dal portachiavi.
    KeyUnavailable,
    /// Connessione rifiutata, host sconosciuto, TLS: l'endpoint non risponde.
    Unreachable,
    Timeout,
    /// L'endpoint ha risposto con uno stato HTTP d'errore.
    Http(u16),
    /// Una risposta che non si capisce (JSON non valido, nessuna scelta).
    BadResponse,
    /// Una risposta senza testo ne' chiamate a tool.
    Empty,
}

/// Il modello dei segretari. `complete` non deve mai andare in panico: ogni
/// fallimento e' un `ModelError`, e deve avere un timeout.
pub trait ChatModel: Send + Sync {
    fn complete(&self, request: ChatRequest<'_>) -> impl Future<Output = Result<ModelReply, ModelError>> + Send;
}
