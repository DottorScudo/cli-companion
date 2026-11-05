//! Quali allegati di un messaggio si scaricano (pura): i primi `ATTACHMENTS_MAX` entro il
//! limite di dimensione dell'HUB; gli altri si elencano per nome e dimensione nel corpo.

use super::mime::AttachmentRef;
use crate::hub::attachments::{IncomingFile, ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};

/// Il nome di un allegato senza nome.
const UNNAMED: &str = "allegato";

/// Gli allegati scelti (indici in `attachments`) e quelli lasciati fuori.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub wanted: Vec<usize>,
    pub not_imported: Vec<(String, u64)>,
}

/// I file di un messaggio: quelli scaricati e quelli non importati (nome, dimensione).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Files {
    pub files: Vec<IncomingFile>,
    pub not_imported: Vec<(String, u64)>,
}

impl Files {
    /// Byte scaricati.
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes.len() as u64).sum()
    }
}

/// Il nome da mostrare di un allegato.
pub fn display_name(attachment: &AttachmentRef) -> String {
    if attachment.filename.trim().is_empty() {
        UNNAMED.to_string()
    } else {
        attachment.filename.clone()
    }
}

pub fn left_out(attachment: &AttachmentRef) -> (String, u64) {
    (display_name(attachment), attachment.size)
}

pub fn select(attachments: &[AttachmentRef]) -> Selection {
    let mut selection = Selection::default();
    for (index, attachment) in attachments.iter().enumerate() {
        if selection.wanted.len() < ATTACHMENTS_MAX && attachment.size <= ATTACHMENT_SIZE_MAX {
            selection.wanted.push(index);
        } else {
            selection.not_imported.push(left_out(attachment));
        }
    }
    selection
}

/// La dimensione dichiarata degli allegati scelti.
pub fn wanted_bytes(attachments: &[AttachmentRef], selection: &Selection) -> u64 {
    selection.wanted.iter().filter_map(|i| attachments.get(*i)).map(|a| a.size).sum()
}

#[cfg(test)]
mod tests;
