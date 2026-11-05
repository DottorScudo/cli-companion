//! Da un messaggio letto a un elemento della fonte, e l'effetto sul cursore (puri).

use super::attachments::Files;
use super::cursor::RouteCursor;
use super::format::{date_label, email_body, sender_name, thread_title};
use super::message::ParsedMessage;
use crate::connectors::contracts::SourceItem;

/// L'elemento per una regola: id esterno = id del messaggio, conversazione = thread di Gmail.
pub fn build_item(route_id: &str, message: &ParsedMessage, files: &Files) -> SourceItem {
    let sender = sender_name(&message.from);
    let date = date_label(&message.date, message.internal_ms);
    SourceItem {
        route: route_id.to_string(),
        external_id: message.id.clone(),
        conversation_key: message.thread_id.clone(),
        title: thread_title(&message.subject),
        body: email_body(&message.subject, &sender, &date, &files.not_imported, &message.text),
        sender,
        files: files.files.clone(),
    }
}

/// Il messaggio si consegna: la regola lo ricorda e il punto di ripartenza avanza.
pub fn deliver(rc: &mut RouteCursor, route_id: &str, message: &ParsedMessage, files: &Files) -> SourceItem {
    rc.remember(&message.id, message.internal_ms);
    rc.advance(message.internal_ms);
    build_item(route_id, message, files)
}

#[cfg(test)]
mod tests;
