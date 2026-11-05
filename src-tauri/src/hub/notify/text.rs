//! Il corpo di una notifica, puro: "canale · thread" e la prima riga del messaggio.

use crate::hub::channel::Channel;
use crate::hub::text::clean_line;

/// Caratteri al massimo per riga (poi "…"), contati come caratteri unicode.
pub const LINE_MAX: usize = 120;
/// Nome del canale Generale nelle notifiche.
const GENERAL_NAME: &str = "Generale";
/// Nome di un workspace che non esiste piu': mai il suo id.
const UNKNOWN_WORKSPACE_NAME: &str = "Workspace";
const SEPARATOR: &str = " · ";

/// Il nome del canale per l'utente: `workspace_name` e' il nome del workspace
/// del canale, se esiste ancora.
pub fn channel_name(channel: &Channel, workspace_name: Option<&str>) -> String {
    match (channel, workspace_name) {
        (Channel::General, _) => GENERAL_NAME.to_string(),
        (Channel::Workspace(_), Some(name)) => clean_line(name, LINE_MAX),
        (Channel::Workspace(_), None) => UNKNOWN_WORKSPACE_NAME.to_string(),
    }
}

/// Riga 1 `"<canale> · <thread>"`; riga 2 la prima riga non vuota di `message`,
/// pulita e tagliata. Un messaggio vuoto lascia solo la riga 1.
pub fn body(channel_name: &str, thread_title: &str, message: &str) -> String {
    let head = format!("{channel_name}{SEPARATOR}{}", clean_line(thread_title, LINE_MAX));
    match message.lines().map(|line| clean_line(line, LINE_MAX)).find(|line| !line.is_empty()) {
        Some(line) => format!("{head}\n{line}"),
        None => head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_channel_is_named_never_identified() {
        let ws = Channel::Workspace("ws-123-1".into());
        assert_eq!(channel_name(&Channel::General, None), "Generale");
        assert_eq!(channel_name(&ws, Some("Il mio progetto")), "Il mio progetto");
        assert_eq!(channel_name(&ws, None), "Workspace");
    }

    #[test]
    fn the_body_has_the_thread_and_the_first_non_empty_line() {
        assert_eq!(body("Generale", "Piano", "\n  \nPrimo rigo\nSecondo"), "Generale · Piano\nPrimo rigo");
    }

    #[test]
    fn an_empty_message_leaves_only_the_first_line() {
        assert_eq!(body("Generale", "Piano", ""), "Generale · Piano");
        assert_eq!(body("Generale", "Piano", " \n\t\n"), "Generale · Piano");
    }

    #[test]
    fn a_long_line_is_cut_on_a_character_boundary() {
        let long = "è".repeat(LINE_MAX + 30);
        let b = body("G", "T", &long);
        let line = b.lines().nth(1).unwrap();
        assert_eq!(line, format!("{}…", "è".repeat(LINE_MAX)));
        let exact = "x".repeat(LINE_MAX);
        assert_eq!(body("G", "T", &exact), format!("G · T\n{exact}"));
    }

    #[test]
    fn emoji_are_never_split() {
        let b = body("G", "T", &"🙂".repeat(LINE_MAX + 1));
        assert!(b.ends_with(&format!("{}…", "🙂".repeat(LINE_MAX))));
    }

    #[test]
    fn the_title_is_kept_on_one_line() {
        assert_eq!(body("G", "a\nb", "x"), "G · a b\nx");
    }
}
