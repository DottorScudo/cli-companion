//! I thread di un canale e le loro operazioni: la logica pura dell'HUB, usata
//! dai comandi Tauri e dal task degli eventi della board. Niente orologio ne'
//! generatore di id: tempo e id li passa il chiamante.
//!
//! Limiti (vedi docs/HUB.MD): testo di un messaggio 1..=8000 caratteri, titolo
//! 1..=120, al massimo 500 messaggi per thread (si tolgono i piu' vecchi), al
//! massimo 200 thread chiusi per canale (si tolgono i chiusi aggiornati da piu'
//! tempo), al massimo 200 task collegati a un thread.

use super::channel::Channel;
use super::model::{Author, HubEvent, Message, MessageKind, Thread, ThreadState, ThreadSummary};
use super::text::{self, BODY_MAX};
use super::waiting::waiting_user;
use crate::board::valid_id;
use serde::{Deserialize, Serialize};

/// Messaggi tenuti per thread: oltre, si tolgono i piu' vecchi.
pub const MESSAGES_MAX: usize = 500;

pub const THREAD_NOT_FOUND: &str = "thread non trovato";

/// Il contenuto del file dei thread di un canale.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelThreads {
    #[serde(default)]
    pub threads: Vec<Thread>,
}

/// Un messaggio da aggiungere: `kind` si ricava da `event`.
#[derive(Debug, Clone)]
pub struct NewMessage {
    pub id: String,
    pub author: Author,
    pub body: String,
    pub reply_to: Option<String>,
    pub event: Option<HubEvent>,
}

impl ChannelThreads {
    pub fn thread(&self, id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    pub(super) fn thread_mut(&mut self, id: &str) -> Result<&mut Thread, String> {
        self.threads.iter_mut().find(|t| t.id == id).ok_or_else(|| THREAD_NOT_FOUND.to_string())
    }

    /// Riassunti dei thread, dal piu' recente.
    pub fn summaries(&self) -> Vec<ThreadSummary> {
        let mut list: Vec<ThreadSummary> = self.threads.iter().map(summary).collect();
        list.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
        list
    }

    /// Nuovo thread aperto, senza messaggi.
    pub fn create_thread(&mut self, channel: &Channel, id: String, title: &str, now: i64) -> Result<&Thread, String> {
        let title = text::title(title)?;
        if self.thread(&id).is_some() {
            return Err("esiste già un thread con questo id".to_string());
        }
        self.threads.push(Thread {
            id,
            channel: channel.clone(),
            title,
            state: ThreadState::Open,
            task_ids: Vec::new(),
            proposal_id: None,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        });
        Ok(self.threads.last().expect("thread appena aggiunto"))
    }

    /// Aggiunge un messaggio in coda al thread e toglie i piu' vecchi oltre
    /// `MESSAGES_MAX`. `reply_to` deve essere un messaggio dello stesso thread.
    /// Un thread chiuso accetta messaggi (per esempio gli eventi dei suoi task)
    /// e resta chiuso.
    pub fn add_message(&mut self, thread_id: &str, new: NewMessage, now: i64) -> Result<&Message, String> {
        let body = text::body(&new.body)?;
        let author = validated_author(new.author)?;
        if let Some(event) = &new.event {
            validate_event(event)?;
        }
        let thread = self.thread_mut(thread_id)?;
        if thread.messages.iter().any(|m| m.id == new.id) {
            return Err("esiste già un messaggio con questo id".to_string());
        }
        if let Some(target) = &new.reply_to {
            if !thread.messages.iter().any(|m| &m.id == target) {
                return Err("il messaggio a cui rispondi non è in questo thread".to_string());
            }
        }
        let kind = if new.event.is_some() { MessageKind::Event } else { MessageKind::Text };
        thread.messages.push(Message {
            id: new.id,
            author,
            kind,
            body,
            reply_to: new.reply_to,
            event: new.event,
            created_at: now,
        });
        let excess = thread.messages.len().saturating_sub(MESSAGES_MAX);
        thread.messages.drain(..excess);
        thread.updated_at = now;
        Ok(thread.messages.last().expect("messaggio appena aggiunto"))
    }

    /// Toglie i thread che non sono del canale (un file spostato o modificato a
    /// mano): ritorna quanti ne ha tolti.
    pub fn retain_channel(&mut self, channel: &Channel) -> usize {
        let before = self.threads.len();
        self.threads.retain(|t| &t.channel == channel);
        before - self.threads.len()
    }
}

pub fn summary(t: &Thread) -> ThreadSummary {
    ThreadSummary {
        id: t.id.clone(),
        channel: t.channel.clone(),
        title: t.title.clone(),
        state: t.state,
        task_count: t.task_ids.len(),
        message_count: t.messages.len(),
        waiting_user: waiting_user(&t.messages),
        created_at: t.created_at,
        updated_at: t.updated_at,
    }
}

fn validated_author(author: Author) -> Result<Author, String> {
    match author {
        Author::Bot { name } => Ok(Author::Bot { name: text::bot_name(&name)? }),
        other => Ok(other),
    }
}

/// Un evento deve nominare il suo soggetto con un id valido; i testi brevi
/// rispettano i limiti dei messaggi.
fn validate_event(event: &HubEvent) -> Result<(), String> {
    if event.subject().is_none() {
        return Err("l'evento non indica il task o la proposta a cui si riferisce".to_string());
    }
    if event.task_id.as_deref().is_some_and(|id| !valid_id(id)) {
        return Err("id del task non valido nell'evento".to_string());
    }
    for value in [&event.proposal_id, &event.status].into_iter().flatten() {
        if !text::valid_ref_id(value) {
            return Err("valore non valido nell'evento".to_string());
        }
    }
    if event.summary.as_deref().is_some_and(|s| s.chars().count() > BODY_MAX) {
        return Err(format!("il riassunto dell'evento supera i {BODY_MAX} caratteri"));
    }
    Ok(())
}

mod lifecycle;

#[cfg(test)]
mod tests;
