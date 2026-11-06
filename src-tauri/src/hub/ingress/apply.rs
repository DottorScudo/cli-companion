//! La parte pura della consegna: cosa scrivere nei thread del canale (dedupe,
//! thread nuovo o riaperto, messaggio) e chi svegliare. Niente I/O, orologio ne'
//! generatore di id: li passa il chiamante, cosi' si prova senza `AppHandle`.

use super::{DeliveryTarget, ExternalWake};
use crate::hub::channel::Channel;
use crate::hub::model::{Attachment, Author, MessageOrigin, Thread, ThreadState, ThreadSummary};
use crate::hub::text::{clean_line, TITLE_MAX};
use crate::hub::threads::{message_from, summary, ChannelThreads, NewMessage, THREAD_NOT_FOUND};

/// Il titolo di un thread nuovo il cui titolo, ripulito, e' vuoto.
pub const UNTITLED: &str = "Senza titolo";

/// Dove va il messaggio, risolto: l'id del thread e, per un thread da creare, il
/// suo titolo ripulito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub thread_id: String,
    pub new_title: Option<String>,
}

/// Un elemento gia' consegnato: il thread e il messaggio che lo contengono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub thread_id: String,
    pub message_id: String,
}

/// La destinazione: il thread indicato o uno nuovo con id `new_id`. Il titolo
/// lascia il posto a "…" per stare in `TITLE_MAX` caratteri, come richiede
/// `text::title`.
pub fn destination(target: &DeliveryTarget, new_id: &str) -> Destination {
    match target {
        DeliveryTarget::Existing(id) => Destination { thread_id: id.clone(), new_title: None },
        DeliveryTarget::New { title } => {
            let clean = clean_line(title, TITLE_MAX - 1);
            let title = if clean.is_empty() { UNTITLED.to_string() } else { clean };
            Destination { thread_id: new_id.to_string(), new_title: Some(title) }
        }
    }
}

/// Il messaggio del thread che ha la stessa origine: l'elemento e' gia' stato consegnato.
pub fn find_duplicate(thread: &Thread, origin: &MessageOrigin) -> Option<String> {
    message_from(thread, origin).map(|m| m.id.clone())
}

/// L'elemento con la stessa origine in un thread qualunque del canale.
pub fn find_in_channel(list: &ChannelThreads, origin: &MessageOrigin) -> Option<Delivered> {
    list.threads
        .iter()
        .find_map(|t| find_duplicate(t, origin).map(|message_id| Delivered { thread_id: t.id.clone(), message_id }))
}

/// L'elemento e' gia' stato consegnato? Per un thread esistente (che deve
/// esserci) si guarda in quel thread; per un thread da creare in tutto il canale,
/// cosi' una consegna ripetuta (stato non salvato prima di un riavvio) non apre
/// un secondo thread.
pub fn precheck(
    list: &ChannelThreads,
    destination: &Destination,
    origin: &MessageOrigin,
) -> Result<Option<Delivered>, String> {
    if destination.new_title.is_some() {
        return Ok(find_in_channel(list, origin));
    }
    let thread_id = &destination.thread_id;
    let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok(find_duplicate(thread, origin).map(|message_id| Delivered { thread_id: thread_id.clone(), message_id }))
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
    /// Con `new_title` il thread va creato (e deve non esserci).
    pub destination: &'a Destination,
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
    /// Il canale aveva gia' l'elemento: nulla e' cambiato.
    Duplicate(Delivered),
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

/// Scrive la consegna: ricontrolla il dedupe (un'altra consegna puo' essere
/// passata dopo la lettura), crea il thread se e' nuovo, riapre un thread chiuso
/// e aggiunge il messaggio. In caso di errore la lista puo' essere mezza
/// modificata: chi chiama lavora su una copia e la butta (`HubState::update_when`).
pub fn apply(list: &mut ChannelThreads, write: Write<'_>) -> Result<Applied, String> {
    if let Some(delivered) = precheck(list, write.destination, &write.origin)? {
        return Ok(Applied::Duplicate(delivered));
    }
    let thread_id = write.destination.thread_id.as_str();
    if let Some(title) = &write.destination.new_title {
        list.create_thread(write.channel, thread_id.to_string(), title, write.now)?;
    }
    list.set_state(thread_id, ThreadState::Open, write.now)?;
    let body = write.body.clone();
    let new = NewMessage {
        id: write.message_id,
        author: write.author,
        body: write.body,
        reply_to: None,
        event: None,
        origin: Some(write.origin),
    };
    let sender = match &list.add_message_with_attachments(thread_id, new, write.attachments, write.now)?.author {
        Author::External { name, .. } => name.clone(),
        Author::User | Author::System | Author::Bot { .. } => String::new(),
    };
    let thread = list.thread(thread_id).ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok(Applied::Stored(Stored { summary: summary(thread), body, sender, secretary: wake_target(thread, write.wake) }))
}

#[cfg(test)]
mod tests;
