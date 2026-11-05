//! La parte pura della consegna: cosa scrivere nei thread del canale (dedupe,
//! thread nuovo o riaperto, messaggio) e chi svegliare. Niente I/O, orologio ne'
//! generatore di id: li passa il chiamante, cosi' si prova senza `AppHandle`.

use super::{DeliveryTarget, ExternalWake};
use crate::hub::channel::Channel;
use crate::hub::model::{Attachment, Author, MessageOrigin, Thread, ThreadState, ThreadSummary};
use crate::hub::text::{clean_line, TITLE_MAX};
use crate::hub::threads::{summary, ChannelThreads, NewMessage, THREAD_NOT_FOUND};

/// Il titolo di un thread nuovo il cui titolo, ripulito, e' vuoto.
pub const UNTITLED: &str = "Senza titolo";

/// Dove va il messaggio: l'id del thread (quello indicato o `new_id`) e, per un
/// thread nuovo, il suo titolo ripulito. Il titolo lascia il posto a "…" per
/// stare in `TITLE_MAX` caratteri, come richiede `text::title`.
pub fn destination(target: &DeliveryTarget, new_id: &str) -> (String, Option<String>) {
    match target {
        DeliveryTarget::Existing(id) => (id.clone(), None),
        DeliveryTarget::New { title } => {
            let clean = clean_line(title, TITLE_MAX - 1);
            let title = if clean.is_empty() { UNTITLED.to_string() } else { clean };
            (new_id.to_string(), Some(title))
        }
    }
}

/// Il messaggio del thread che ha la stessa origine (`instance` ed
/// `external_id`): l'elemento e' gia' stato consegnato.
pub fn find_duplicate(thread: &Thread, origin: &MessageOrigin) -> Option<String> {
    thread
        .messages
        .iter()
        .find(|m| {
            m.origin.as_ref().is_some_and(|o| o.instance == origin.instance && o.external_id == origin.external_id)
        })
        .map(|m| m.id.clone())
}

/// Lettura prima di preparare gli allegati: l'id del messaggio gia' consegnato
/// nel thread `thread_id`; il thread deve esserci.
pub fn precheck(list: &ChannelThreads, thread_id: &str, origin: &MessageOrigin) -> Result<Option<String>, String> {
    let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok(find_duplicate(thread, origin))
}

/// Chi svegliare dopo la consegna: il titolare del thread o il segretario
/// indicato; `None` se nessuno (anche un `Owner` senza titolare).
pub fn wake_target(thread: &Thread, wake: &ExternalWake) -> Option<String> {
    match wake {
        ExternalWake::None => None,
        ExternalWake::Owner => thread.owner.clone(),
        ExternalWake::Secretary(name) => Some(name.clone()),
    }
}

/// Tutto quello che serve a scrivere una consegna.
pub struct Write<'a> {
    pub channel: &'a Channel,
    pub thread_id: &'a str,
    /// `Some(titolo)`: il thread va creato (e deve non esserci).
    pub new_title: Option<&'a str>,
    pub author: Author,
    pub origin: MessageOrigin,
    pub body: String,
    pub attachments: Vec<Attachment>,
    pub message_id: String,
    pub wake: &'a ExternalWake,
    pub now: i64,
}

/// L'esito della scrittura.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// Il thread aveva gia' l'elemento: nulla e' cambiato.
    Duplicate {
        message_id: String,
    },
    Stored(Stored),
}

/// Il messaggio scritto, con quello che serve dopo il salvataggio.
#[derive(Debug, PartialEq, Eq)]
pub struct Stored {
    pub summary: ThreadSummary,
    /// Il corpo cosi' come e' stato salvato (per la notifica).
    pub body: String,
    /// Come si presenta il mittente (per la notifica).
    pub sender: String,
    /// Il segretario da svegliare, se c'e'.
    pub secretary: Option<String>,
}

impl Applied {
    /// Va salvato solo un messaggio nuovo.
    pub fn changed(&self) -> bool {
        matches!(self, Self::Stored(_))
    }
}

/// Scrive la consegna: crea il thread se e' nuovo, ricontrolla il dedupe (un'altra
/// consegna puo' essere passata dopo la lettura), riapre un thread chiuso e
/// aggiunge il messaggio. In caso di errore la lista puo' essere mezza
/// modificata: chi chiama lavora su una copia e la butta (`HubState::update_when`).
pub fn apply(list: &mut ChannelThreads, write: Write<'_>) -> Result<Applied, String> {
    if let Some(title) = write.new_title {
        list.create_thread(write.channel, write.thread_id.to_string(), title, write.now)?;
    } else if let Some(message_id) = precheck(list, write.thread_id, &write.origin)? {
        return Ok(Applied::Duplicate { message_id });
    }
    list.set_state(write.thread_id, ThreadState::Open, write.now)?;
    let body = write.body.clone();
    let new = NewMessage {
        id: write.message_id,
        author: write.author,
        body: write.body,
        reply_to: None,
        event: None,
        origin: Some(write.origin),
    };
    let sender = match &list.add_message_with_attachments(write.thread_id, new, write.attachments, write.now)?.author {
        Author::External { name, .. } => name.clone(),
        Author::User | Author::System | Author::Bot { .. } => String::new(),
    };
    let thread = list.thread(write.thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok(Applied::Stored(Stored { summary: summary(thread), body, sender, secretary: wake_target(thread, write.wake) }))
}

#[cfg(test)]
mod tests;
