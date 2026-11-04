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
