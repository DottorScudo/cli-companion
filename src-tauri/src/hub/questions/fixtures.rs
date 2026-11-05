//! Dati di prova per le domande dei worker: thread, eventi e una board finta
//! che registra le risposte consegnate.

use super::{QuestionBoard, QuestionRef};
use crate::hub::channel::Channel;
use crate::hub::model::{Author, HubEvent, HubEventKind, HubQuestion, Message, MessageKind, Thread, ThreadState};
use std::sync::Mutex;

pub const WS: &str = "ws-1";

/// Messaggio di sistema `id` con l'evento `event` del workspace di prova.
pub fn event_message(id: &str, event: HubEvent) -> Message {
    Message {
        id: id.to_string(),
        author: Author::System,
        kind: MessageKind::Event,
        body: "evento".to_string(),
        reply_to: None,
        event: Some(HubEvent { workspace: Some(WS.to_string()), ..event }),
        attachments: Vec::new(),
        origin: None,
        created_at: 0,
    }
}

/// Evento "domanda" `question_id` del task `task_id`.
pub fn question_event(id: &str, task_id: &str, question_id: &str) -> Message {
    let question = HubQuestion { id: question_id.to_string(), text: "Quale database?".to_string(), options: vec![] };
    event_message(id, HubEvent { question: Some(question), ..HubEvent::of_task(HubEventKind::TaskQuestion, task_id) })
}

/// Evento `kind` del task `task_id`, senza domanda.
pub fn task_event(id: &str, kind: HubEventKind, task_id: &str) -> Message {
    event_message(id, HubEvent::of_task(kind, task_id))
}

/// Messaggio dell'utente `id`, in risposta a `to` se c'e'.
pub fn user(id: &str, body: &str, to: Option<&str>) -> Message {
    Message {
        id: id.to_string(),
        author: Author::User,
        kind: MessageKind::Text,
        body: body.to_string(),
        reply_to: to.map(str::to_string),
        event: None,
        attachments: Vec::new(),
        origin: None,
        created_at: 0,
    }
}

/// Thread `t1` del workspace di prova con i messaggi dati.
pub fn thread(messages: Vec<Message>) -> Thread {
    Thread {
        id: "t1".to_string(),
        channel: Channel::Workspace(WS.to_string()),
        title: "Login".to_string(),
        state: ThreadState::Open,
        task_ids: vec![],
        proposal_id: None,
        owner: None,
        created_at: 0,
        updated_at: 0,
        summary: None,
        messages,
    }
}

pub fn question_ref(task_id: &str, question_id: &str) -> QuestionRef {
    QuestionRef { workspace: WS.to_string(), task_id: task_id.to_string(), question_id: question_id.to_string() }
}

/// Board finta: registra le risposte; con `refuse` le rifiuta con quell'errore.
#[derive(Default)]
pub struct FakeAnswers {
    pub delivered: Mutex<Vec<(QuestionRef, String)>>,
    pub refuse: Option<String>,
}

impl FakeAnswers {
    pub fn refusing(reason: &str) -> Self {
        Self { refuse: Some(reason.to_string()), ..Self::default() }
    }

    pub fn delivered(&self) -> Vec<(QuestionRef, String)> {
        self.delivered.lock().unwrap().clone()
    }
}

impl QuestionBoard for FakeAnswers {
    fn answer(&self, question: &QuestionRef, text: &str) -> Result<(), String> {
        if let Some(reason) = &self.refuse {
            return Err(reason.clone());
        }
        self.delivered.lock().unwrap().push((question.clone(), text.to_string()));
        Ok(())
    }
}
