//! Il ciclo di tool calling di un turno: conversazione -> chiamata al modello
//! -> se il modello chiede dei tool, il codice li valida (`tools::parse`), li
//! esegue attraverso `TurnHost` e restituisce l'esito; altrimenti il testo e'
//! la risposta del segretario. Al massimo `max_steps` chiamate al modello e
//! `audit::MAX_MUTATIONS_PER_TURN` modifiche per turno. Qui anche la
//! compattazione della conversazione di una sessione e la richiesta del
//! riassunto alla sua chiusura.
//!
//! Il ciclo non tiene lock: tutto cio' che tocca thread e board passa da
//! `TurnHost`, che il runtime implementa sull'app e i test con un finto.

use super::audit::{self, MutationBudget};
use super::board_query::BoardQuery;
use super::chat::{ChatMessage, ChatModel, ChatRequest, ModelError, ModelReply, ToolCall};
use super::context;
use super::relay::QuestionSelector;
use super::texts;
use super::thread_page::ThreadPage;
use super::tools::{self, Invocation};
use crate::hub::settings::Secretary;
use crate::validate::clip_with_ellipsis;
use serde_json::{json, Value};
use std::future::Future;

/// Caratteri di un esito di tool restituito al modello nel turno in corso.
pub const TOOL_RESULT_MAX: usize = 6000;
/// Caratteri tenuti degli esiti dei tool dei turni passati della sessione.
pub const TOOL_RESULT_KEPT: usize = 600;
/// Messaggi tenuti nella conversazione di una sessione: oltre, si tolgono i
/// piu' vecchi (fino a un messaggio dell'utente, cosi' nessun esito di tool
/// resta senza la sua chiamata).
pub const CONVERSATION_MAX: usize = 40;

/// Gli effetti di un turno. Gli errori sono in inglese: tornano al modello.
pub trait TurnHost: Send + Sync {
    /// Una pagina della board del workspace (`None` = quella del canale del thread).
    fn board_status(
        &self,
        workspace: Option<String>,
        query: BoardQuery,
    ) -> impl Future<Output = Result<Value, String>> + Send;
    /// Il riepilogo di tutti i workspace che il segretario puo' consultare.
    fn board_overview(&self) -> impl Future<Output = Result<Value, String>> + Send;
    /// Un task per intero, della board del workspace (`None` = quella del canale del thread).
    fn task_details(
        &self,
        workspace: Option<String>,
        task_id: String,
    ) -> impl Future<Output = Result<Value, String>> + Send;
    /// Il thread compatto, con la pagina di messaggi chiesta.
    fn thread_read(&self, page: ThreadPage) -> impl Future<Output = Result<Value, String>> + Send;
    /// Scrive un messaggio del segretario nel thread.
    fn post_message(&self, body: String) -> impl Future<Output = Result<(), String>> + Send;
    /// Apre un task di pianificazione collegato al thread (vedi `planning::open_plan`).
    fn open_plan_task(&self, goal: String) -> impl Future<Output = Result<Value, String>> + Send;
    /// Decide la proposta a cui risponde il messaggio del turno (vedi `planning::decide`).
    fn decide_proposal(&self, accept: bool) -> impl Future<Output = Result<Value, String>> + Send;
    /// Consegna la risposta alla domanda in attesa scelta dal selettore (vedi `relay::relay`).
    fn relay_answer(
        &self,
        selector: QuestionSelector,
        body: String,
    ) -> impl Future<Output = Result<Value, String>> + Send;
    /// Scrive un messaggio di sistema (in italiano) nel thread.
    fn notice(&self, text: String) -> impl Future<Output = ()> + Send;
}

/// Il segretario del turno, il suo system prompt completo e il limite di passi.
pub struct TurnSpec<'a> {
    pub secretary: &'a Secretary,
    pub system: String,
    pub max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEnd {
    /// La risposta finale, da scrivere nel thread.
    Reply(String),
    /// Raggiunto `max_steps` con il modello che chiedeva ancora dei tool.
    StepLimit,
    Failed(ModelError),
}

/// Esegue il turno sulla conversazione, che deve terminare con il messaggio
/// dell'utente che lo apre. La conversazione resta sempre valida: le chiamate
/// a tool dell'ultimo passo, che non si eseguono piu', non vi entrano.
pub async fn run_turn<M: ChatModel, H: TurnHost>(
    model: &M,
    host: &H,
    spec: &TurnSpec<'_>,
    conversation: &mut Vec<ChatMessage>,
) -> TurnEnd {
    let tools = tools::specs(spec.secretary);
    let mut budget = MutationBudget::default();
    for step in 1..=spec.max_steps {
        let request = ChatRequest { system: &spec.system, messages: conversation, tools: &tools };
        let (text, calls) = match model.complete(request).await {
            Err(e) => return TurnEnd::Failed(e),
            Ok(ModelReply::Text(text)) => {
                conversation.push(ChatMessage::Assistant { text: Some(text.clone()), tool_calls: Vec::new() });
                return TurnEnd::Reply(text);
            }
            Ok(ModelReply::ToolCalls { text, calls }) => (text, calls),
        };
        if step == spec.max_steps {
            break;
        }
        conversation.push(ChatMessage::Assistant { text, tool_calls: calls.clone() });
        for call in &calls {
            let content = execute(host, spec.secretary, call, &mut budget).await;
            conversation.push(ChatMessage::Tool {
                call_id: call.id.clone(),
                content: clip_with_ellipsis(&content, TOOL_RESULT_MAX),
            });
        }
    }
    TurnEnd::StepLimit
}

/// Valida ed esegue una chiamata entro il limite di modifiche del turno;
/// l'esito (o l'errore) e' JSON per il modello. Un tool rifiutato e' anche un
/// messaggio di sistema nel thread, come una modifica riuscita che non vi si
/// vede gia' (`audit::notice`).
async fn execute<H: TurnHost>(host: &H, secretary: &Secretary, call: &ToolCall, budget: &mut MutationBudget) -> String {
    let admitted = tools::parse(call, secretary).and_then(|invocation| {
        budget.admit(invocation.tool())?;
        Ok(invocation)
    });
    let outcome = match admitted {
        Err(rejection) => {
            host.notice(texts::tool_rejected(&secretary.name, &rejection)).await;
            Err(rejection.for_model())
        }
        Ok(invocation) => {
            let audited = invocation.clone();
            let outcome = invoke(host, invocation).await;
            if let Ok(value) = &outcome {
                budget.record(audited.tool());
                if let Some(text) = audit::notice(&secretary.name, &audited, value) {
                    host.notice(text).await;
                }
            }
            outcome
        }
    };
    match outcome {
        Ok(value) => value.to_string(),
        Err(error) => json!({ "error": error }).to_string(),
    }
}

/// Esegue una chiamata validata attraverso l'host.
async fn invoke<H: TurnHost>(host: &H, invocation: Invocation) -> Result<Value, String> {
    match invocation {
        Invocation::BoardStatus { workspace, query } => host.board_status(workspace, query).await,
        Invocation::BoardOverview => host.board_overview().await,
        Invocation::TaskDetails { workspace, task_id } => host.task_details(workspace, task_id).await,
        Invocation::ThreadRead { page } => host.thread_read(page).await,
        Invocation::PostMessage { body } => host.post_message(body).await.map(|()| json!({ "posted": true })),
        Invocation::OpenPlanTask { goal } => host.open_plan_task(goal).await,
        Invocation::DecideProposal { accept } => host.decide_proposal(accept).await,
        Invocation::RelayAnswer { body, selector } => host.relay_answer(selector, body).await,
    }
}

/// Dopo un turno: esiti dei tool accorciati e al massimo `CONVERSATION_MAX`
/// messaggi, a partire da un messaggio dell'utente.
pub fn compact(conversation: &mut Vec<ChatMessage>) {
    for message in conversation.iter_mut() {
        if let ChatMessage::Tool { content, .. } = message {
            if content.chars().count() > TOOL_RESULT_KEPT {
                *content = clip_with_ellipsis(content, TOOL_RESULT_KEPT);
            }
        }
    }
    if conversation.len() <= CONVERSATION_MAX {
        return;
    }
    let mut start = conversation.len() - CONVERSATION_MAX;
    while start < conversation.len() && !matches!(conversation[start], ChatMessage::User(_)) {
        start += 1;
    }
    conversation.drain(..start);
}

/// Il riassunto aggiornato del thread alla chiusura di una sessione, chiesto
/// al modello senza tool. `None` se la chiamata fallisce o non resta nulla:
/// il chiamante tiene il riassunto precedente.
pub async fn updated_summary<M: ChatModel>(
    model: &M,
    previous: Option<&str>,
    conversation: &[ChatMessage],
) -> Option<String> {
    let messages = [ChatMessage::User(context::summary_message(previous, conversation))];
    let request = ChatRequest { system: context::summary_system(), messages: &messages, tools: &[] };
    match model.complete(request).await {
        Ok(ModelReply::Text(text)) => context::clean_summary(&text),
        Ok(ModelReply::ToolCalls { .. }) => {
            log::warn!("hub: il modello ha chiesto dei tool invece del riassunto, tengo il precedente");
            None
        }
        Err(e) => {
            log::warn!("hub: riassunto del thread non aggiornato ({e:?}), tengo il precedente");
            None
        }
    }
}

#[cfg(test)]
mod tests;
