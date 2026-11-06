//! Dai messaggi di Telegram ai messaggi in entrata del runtime (puro): gli album si
//! fondono in un solo messaggio e il testo diventa comando o invito di abbinamento.

use super::commands;
use super::ids::external_id;
use super::updates::{ChatMessage, Update, UpdateKind};
use crate::connectors::contracts::InboundMessage;

fn same_album(a: &ChatMessage, b: &ChatMessage) -> bool {
    a.chat_id == b.chat_id && a.media_group.is_some() && a.media_group == b.media_group
}

/// Unisce i messaggi consecutivi dello stesso album: ids del primo, allegati accodati,
/// la prima didascalia non vuota.
pub fn merge_albums(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut merged: Vec<ChatMessage> = Vec::new();
    for message in messages {
        match merged.last_mut() {
            Some(first) if same_album(first, &message) => {
                first.files.extend(message.files);
                if first.caption.is_none() {
                    first.caption = message.caption;
                }
                if first.text.is_none() {
                    first.text = message.text;
                }
            }
            _ => merged.push(message),
        }
    }
    merged
}

/// Un messaggio di Telegram come messaggio in entrata. I comandi si riconoscono
/// solo nel testo, non nella didascalia.
pub fn to_inbound(message: ChatMessage) -> InboundMessage {
    let parsed = message.text.as_deref().map(commands::parse).unwrap_or_default();
    let text = message.text.or(message.caption).unwrap_or_default();
    InboundMessage {
        external_id: external_id(message.chat_id, message.message_id),
        account: message.sender_id.to_string(),
        chat: message.chat_id.to_string(),
        subchat: None,
        sender_name: message.sender_name,
        text,
        reply_to_external: message.reply_to.map(|id| external_id(message.chat_id, id)),
        files: message.files,
        pairing_code: parsed.pairing_code,
        command: parsed.command,
    }
}

/// I messaggi in entrata di un giro, nell'ordine di arrivo.
pub fn build_messages(updates: &[Update]) -> Vec<InboundMessage> {
    let messages = updates
        .iter()
        .filter_map(|update| match &update.kind {
            UpdateKind::Message(message) => Some(message.clone()),
            _ => None,
        })
        .collect();
    merge_albums(messages).into_iter().map(to_inbound).collect()
}

#[cfg(test)]
mod tests;
