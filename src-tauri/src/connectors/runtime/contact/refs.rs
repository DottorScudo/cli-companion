//! Il riferimento a un messaggio dell'HUB che sta dietro un messaggio esterno:
//! canale, thread e id del messaggio. E' il valore di `external_to_hub` (chiave:
//! l'id esterno), cosi' una risposta a una notifica sa in quale thread scrivere.
//! Pura. Il formato su disco sono le tre parti separate da uno spazio; un valore
//! vecchio (solo l'id del messaggio) o malformato non si decodifica (`None`).

use crate::hub::{valid_ref_id, Channel};

const SEPARATOR: char = ' ';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRef {
    pub channel: Channel,
    pub thread_id: String,
    pub message_id: String,
}

impl MessageRef {
    pub fn encode(&self) -> String {
        format!("{}{SEPARATOR}{}{SEPARATOR}{}", self.channel.as_str(), self.thread_id, self.message_id)
    }

    /// `None` se non sono esattamente tre parti valide.
    pub fn decode(raw: &str) -> Option<Self> {
        let mut parts = raw.split(SEPARATOR);
        let (channel, thread_id, message_id) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || !valid_ref_id(thread_id) || !valid_ref_id(message_id) {
            return None;
        }
        let channel = Channel::parse(channel).ok()?;
        Some(Self { channel, thread_id: thread_id.to_string(), message_id: message_id.to_string() })
    }
}

#[cfg(test)]
mod tests;
