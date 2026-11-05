//! Quale pagina di messaggi di un thread legge `thread_read` (pura, senza app):
//! gli ultimi, oppure quelli prima di un messaggio o dopo un certo numero di
//! messaggi recenti. Il resto dell'output e' in `output`.

use crate::hub::model::Message;
use std::ops::Range;

/// Messaggi di una pagina di `thread_read`.
pub const THREAD_PAGE_SIZE: usize = 20;
/// Caratteri massimi dell'id di un messaggio nei parametri del tool.
pub const MESSAGE_ID_MAX: usize = 64;

/// Dove si ferma la pagina letta, partendo dalla fine del thread.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ThreadPage {
    /// Gli ultimi messaggi.
    #[default]
    Latest,
    /// I messaggi immediatamente prima di quello con questo id.
    Before(String),
    /// I messaggi prima degli ultimi `n`, che si saltano.
    Offset(usize),
    /// Il testo di un solo messaggio, a partire dal carattere `offset` (non e' una pagina di messaggi).
    Body { message_id: String, offset: usize },
}

/// L'intervallo dei messaggi della pagina; l'errore e' in inglese, per il modello.
pub fn window(messages: &[Message], page: &ThreadPage) -> Result<Range<usize>, String> {
    let end = match page {
        ThreadPage::Latest => messages.len(),
        ThreadPage::Offset(skipped) if *skipped == 0 || *skipped < messages.len() => messages.len() - skipped,
        ThreadPage::Offset(skipped) => {
            return Err(format!("`offset` {skipped} is beyond the thread, which has {} messages", messages.len()));
        }
        ThreadPage::Before(id) => messages
            .iter()
            .position(|m| m.id == *id)
            .ok_or_else(|| "`before` is not the id of a message of this thread".to_string())?,
        ThreadPage::Body { .. } => return Err("`message_id` reads one message, not a page".to_string()),
    };
    Ok(end.saturating_sub(THREAD_PAGE_SIZE)..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::numbered;

    #[test]
    fn the_latest_page_ends_at_the_last_message() {
        assert_eq!(window(&numbered(25), &ThreadPage::Latest), Ok(5..25));
        assert_eq!(window(&numbered(3), &ThreadPage::Latest), Ok(0..3));
        assert_eq!(window(&[], &ThreadPage::Latest), Ok(0..0));
    }

    #[test]
    fn an_offset_skips_the_most_recent_messages() {
        assert_eq!(window(&numbered(50), &ThreadPage::Offset(20)), Ok(10..30));
        assert_eq!(window(&numbered(50), &ThreadPage::Offset(40)), Ok(0..10));
        assert_eq!(window(&numbered(5), &ThreadPage::Offset(0)), Ok(0..5));
        assert!(window(&numbered(5), &ThreadPage::Offset(5)).unwrap_err().contains("beyond"));
        assert!(window(&[], &ThreadPage::Offset(1)).is_err());
    }

    #[test]
    fn a_message_body_is_not_a_page() {
        let body = ThreadPage::Body { message_id: "m1".into(), offset: 0 };
        assert!(window(&numbered(5), &body).unwrap_err().contains("not a page"));
    }

    #[test]
    fn before_stops_just_ahead_of_the_message() {
        assert_eq!(window(&numbered(50), &ThreadPage::Before("m30".into())), Ok(10..30));
        assert_eq!(window(&numbered(50), &ThreadPage::Before("m3".into())), Ok(0..3));
        assert_eq!(window(&numbered(50), &ThreadPage::Before("m0".into())), Ok(0..0));
        assert!(window(&numbered(5), &ThreadPage::Before("nope".into())).is_err());
    }
}
