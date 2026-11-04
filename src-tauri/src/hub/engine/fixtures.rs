//! Dati di prova per i test del motore: segretari, messaggi e thread.

use crate::hub::channel::Channel;
use crate::hub::model::{Author, Message, MessageKind, Thread, ThreadState};
use crate::hub::settings::{Secretary, SecretaryChannels, SecretaryTool};

pub const WS: &str = "ws-1";

pub fn ws() -> Channel {
    Channel::Workspace(WS.to_string())
}

/// Segretario configurato, in tutti i canali, con tutti i tool.
pub fn secretary(name: &str) -> Secretary {
    Secretary {
        name: name.to_string(),
        image: None,
        system_prompt: format!("You are {name}. Report the board status."),
        tools: SecretaryTool::ALL.to_vec(),
        channels: SecretaryChannels::All,
    }
}

pub fn msg(id: &str, author: Author, body: &str, at: i64) -> Message {
    Message {
        id: id.to_string(),
        author,
        kind: MessageKind::Text,
        body: body.to_string(),
        reply_to: None,
        event: None,
        created_at: at,
    }
}

pub fn user(id: &str, body: &str) -> Message {
    msg(id, Author::User, body, 0)
}

pub fn bot(id: &str, name: &str, body: &str) -> Message {
    msg(id, Author::Bot { name: name.to_string() }, body, 0)
}

/// Thread `t1` del workspace di prova con i messaggi dati.
pub fn thread(messages: Vec<Message>) -> Thread {
    Thread {
        id: "t1".to_string(),
        channel: ws(),
        title: "Login".to_string(),
        state: ThreadState::Open,
        task_ids: vec!["t-1".to_string()],
        proposal_id: None,
        owner: None,
        created_at: 0,
        updated_at: 0,
        summary: None,
        messages,
    }
}

/// `n` messaggi dell'utente `m0..m<n>`.
pub fn numbered(n: usize) -> Vec<Message> {
    (0..n).map(|i| msg(&format!("m{i}"), Author::User, &format!("messaggio {i}"), i as i64)).collect()
}

/// Board finta per `decide_proposal`: le proposte pendenti (workspace, id,
/// numero di task) e le decisioni ricevute.
#[derive(Default)]
pub struct FakeBoard {
    pub pending: std::sync::Mutex<Vec<(String, String, usize)>>,
    pub decisions: std::sync::Mutex<Vec<(String, String, bool)>>,
    /// Errore della board alla decisione (per esempio un id gia' occupato).
    pub refuse: Option<String>,
}

impl FakeBoard {
    pub fn with_pending(workspace: &str, proposal_id: &str, tasks: usize) -> Self {
        let board = Self::default();
        board.pending.lock().unwrap().push((workspace.to_string(), proposal_id.to_string(), tasks));
        board
    }
}

impl crate::hub::engine::planning::ProposalBoard for FakeBoard {
    fn pending_tasks(&self, workspace: &str, proposal_id: &str) -> Option<usize> {
        let pending = self.pending.lock().unwrap();
        pending.iter().find(|(ws, id, _)| ws == workspace && id == proposal_id).map(|(_, _, n)| *n)
    }

    fn decide(&self, workspace: &str, proposal_id: &str, accept: bool) -> Result<(), String> {
        if let Some(e) = &self.refuse {
            return Err(e.clone());
        }
        self.pending.lock().unwrap().retain(|(ws, id, _)| !(ws == workspace && id == proposal_id));
        self.decisions.lock().unwrap().push((workspace.to_string(), proposal_id.to_string(), accept));
        Ok(())
    }
}

/// Messaggio evento "proposta pronta" `id` della proposta `proposal_id` nel
/// workspace di prova.
pub fn proposal_event(id: &str, proposal_id: &str) -> Message {
    let mut m = msg(id, Author::System, "Nuova proposta: 2 task\n- Login\n- Logout", 0);
    m.kind = MessageKind::Event;
    m.event = Some(crate::hub::model::HubEvent {
        workspace: Some(WS.to_string()),
        task_id: Some("plan-login".to_string()),
        ..crate::hub::model::HubEvent::of_proposal(crate::hub::model::HubEventKind::ProposalCreated, proposal_id)
    });
    m
}

/// Messaggio dell'utente `id` in risposta al messaggio `to`.
pub fn reply_to(id: &str, to: &str, body: &str) -> Message {
    Message { reply_to: Some(to.to_string()), ..user(id, body) }
}
