//! I thread di un canale e le loro operazioni: la logica pura dell'HUB, usata
//! dai comandi Tauri e dal task degli eventi della board. Niente orologio ne'
//! generatore di id: tempo e id li passa il chiamante.
//!
//! Limiti (vedi docs/HUB.MD): testo di un messaggio 1..=8000 caratteri, titolo
//! 1..=120, al massimo 500 messaggi per thread (si tolgono i piu' vecchi), al
//! massimo 200 thread chiusi per canale (si tolgono i chiusi aggiornati da piu'
//! tempo), al massimo 200 task collegati a un thread; gli allegati hanno i limiti
//! di `attachments` (con almeno un allegato il testo puo' essere vuoto).

use super::attachments;
use super::channel::Channel;
use super::model::{
    Attachment, Author, HubActionRequest, HubEvent, HubQuestion, Message, MessageKind, MessageOrigin, Thread,
    ThreadState, ThreadSummary,
};
use super::text::{self, BODY_MAX};
use super::waiting::waiting_user;
use crate::board::{valid_id, OPTIONS_MAX, OPTION_MAX, QUESTION_MAX};
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
    /// Da dove viene, se arriva da fuori dell'app (vedi `MessageOrigin`).
    pub origin: Option<MessageOrigin>,
}

impl ChannelThreads {
    pub fn thread(&self, id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    pub(super) fn thread_mut(&mut self, id: &str) -> Result<&mut Thread, String> {
        self.threads.iter_mut().find(|t| t.id == id).ok_or_else(|| THREAD_NOT_FOUND.to_string())
    }

    /// L'allegato `attachment_id` di un messaggio del thread.
    pub fn attachment(&self, thread_id: &str, attachment_id: &str) -> Result<&Attachment, String> {
        let thread = self.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
        thread
            .messages
            .iter()
            .flat_map(|m| &m.attachments)
            .find(|a| a.id == attachment_id)
            .ok_or_else(|| attachments::NOT_FOUND.to_string())
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
            proposal_ids: Vec::new(),
            owner: None,
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
        });
        Ok(self.threads.last().expect("thread appena aggiunto"))
    }

    /// Aggiunge un messaggio in coda al thread e toglie i piu' vecchi oltre
    /// `MESSAGES_MAX`. `reply_to` deve essere un messaggio dello stesso thread.
    /// Un thread chiuso accetta messaggi (per esempio gli eventi dei suoi task)
    /// e resta chiuso.
    pub fn add_message(&mut self, thread_id: &str, new: NewMessage, now: i64) -> Result<&Message, String> {
        self.add_message_with_attachments(thread_id, new, Vec::new(), now)
    }

    /// Come `add_message`, con gli allegati (solo i metadati, gia' nella
    /// cartella del thread). Solo l'utente e un mittente esterno possono allegare; con almeno un
    /// allegato il testo puo' essere vuoto.
    pub fn add_message_with_attachments(
        &mut self,
        thread_id: &str,
        new: NewMessage,
        attachments: Vec<Attachment>,
        now: i64,
    ) -> Result<&Message, String> {
        let body = if attachments.is_empty() { text::body(&new.body)? } else { text::optional_body(&new.body)? };
        if !attachments.is_empty() && !matches!(new.author, Author::User | Author::External { .. }) {
            return Err("solo i messaggi dell'utente o di un mittente esterno possono avere allegati".to_string());
        }
        attachments::validate(&attachments)?;
        let author = validated_author(new.author)?;
        let origin = validated_origin(new.origin, &author)?;
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
            attachments,
            origin,
            created_at: now,
        });
        let excess = thread.messages.len().saturating_sub(MESSAGES_MAX);
        thread.messages.drain(..excess);
        thread.updated_at = now;
        Ok(thread.messages.last().expect("messaggio appena aggiunto"))
    }

    /// Sostituisce il riassunto del thread (gia' ripulito dal chiamante). Non
    /// tocca `updated_at`: e' un dato interno dei segretari, non una novita'
    /// per l'utente. Ritorna se e' cambiato.
    pub fn set_summary(&mut self, thread_id: &str, summary: String) -> Result<bool, String> {
        let thread = self.thread_mut(thread_id)?;
        if thread.summary.as_deref() == Some(summary.as_str()) {
            return Ok(false);
        }
        thread.summary = Some(summary);
        Ok(true)
    }

    /// Toglie i thread che non sono del canale (un file spostato o modificato a
    /// mano): ritorna quanti ne ha tolti.
    pub fn retain_channel(&mut self, channel: &Channel) -> usize {
        let before = self.threads.len();
        self.threads.retain(|t| &t.channel == channel);
        before - self.threads.len()
    }
}

/// Il messaggio del thread che viene dallo stesso elemento esterno: stessa
/// istanza e stesso `external_id`, la chiave di dedupe di cio' che arriva da fuori.
pub fn message_from<'a>(thread: &'a Thread, origin: &MessageOrigin) -> Option<&'a Message> {
    thread.messages.iter().find(|m| {
        m.origin.as_ref().is_some_and(|o| o.instance == origin.instance && o.external_id == origin.external_id)
    })
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
        owner: t.owner.clone(),
        created_at: t.created_at,
        updated_at: t.updated_at,
    }
}

fn validated_author(author: Author) -> Result<Author, String> {
    match author {
        Author::Bot { name } => Ok(Author::Bot { name: text::bot_name(&name)? }),
        Author::External { connector, instance, name } => {
            check_ref("connettore", &connector)?;
            check_ref("istanza", &instance)?;
            Ok(Author::External { connector, instance, name: text::external_name(&name)? })
        }
        other => Ok(other),
    }
}

/// L'origine di un messaggio: id validi e `external_id` nei limiti. Un autore
/// `external` ne ha sempre una, del suo stesso connettore e della sua stessa
/// istanza.
fn validated_origin(origin: Option<MessageOrigin>, author: &Author) -> Result<Option<MessageOrigin>, String> {
    if let Some(origin) = &origin {
        check_ref("connettore", &origin.connector)?;
        check_ref("istanza", &origin.instance)?;
        text::external_id(&origin.external_id)?;
    }
    if let Author::External { connector, instance, .. } = author {
        let matches = origin.as_ref().is_some_and(|o| &o.connector == connector && &o.instance == instance);
        if !matches {
            return Err("un messaggio esterno richiede un'origine dello stesso connettore e istanza".to_string());
        }
    }
    Ok(origin)
}

fn check_ref(label: &str, value: &str) -> Result<(), String> {
    if text::valid_ref_id(value) {
        Ok(())
    } else {
        Err(format!("id del {label} non valido"))
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
    if event.workspace.as_deref().is_some_and(|ws| !matches!(Channel::parse(ws), Ok(Channel::Workspace(_)))) {
        return Err("workspace non valido nell'evento".to_string());
    }
    for (label, value) in [("il riassunto", &event.summary), ("la nota", &event.note)] {
        if value.as_deref().is_some_and(|s| s.chars().count() > BODY_MAX) {
            return Err(format!("{label} dell'evento supera i {BODY_MAX} caratteri"));
        }
    }
    event.request.as_ref().map_or(Ok(()), validate_request)?;
    event.question.as_ref().map_or(Ok(()), validate_question)
}

/// La richiesta di azione di un evento: id valido e testi nei limiti dei messaggi.
fn validate_request(request: &HubActionRequest) -> Result<(), String> {
    if !text::valid_ref_id(&request.id) {
        return Err("id della richiesta non valido nell'evento".to_string());
    }
    text::bot_name(&request.secretary)?;
    let texts = [Some(&request.reason), Some(&request.task_title), request.note.as_ref()];
    if texts.into_iter().flatten().any(|s| s.chars().count() > BODY_MAX) {
        return Err(format!("un testo della richiesta supera i {BODY_MAX} caratteri"));
    }
    Ok(())
}

/// La domanda di un evento: id valido, testo e opzioni nei limiti di `ask_user`.
fn validate_question(question: &HubQuestion) -> Result<(), String> {
    if !text::valid_ref_id(&question.id) {
        return Err("id della domanda non valido nell'evento".to_string());
    }
    if question.text.chars().count() > QUESTION_MAX {
        return Err(format!("la domanda dell'evento supera i {QUESTION_MAX} caratteri"));
    }
    let option_too_long = question.options.iter().any(|o| o.chars().count() > OPTION_MAX);
    if question.options.len() > OPTIONS_MAX || option_too_long {
        return Err("opzioni della domanda non valide nell'evento".to_string());
    }
    Ok(())
}

pub(super) mod lifecycle;
mod requests;

pub use lifecycle::TASKS_PER_THREAD_MAX;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod external_tests;

#[cfg(test)]
mod removal_tests;
