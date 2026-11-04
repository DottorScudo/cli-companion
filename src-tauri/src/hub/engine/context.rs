//! Il contesto che il modello riceve (puro), sempre in inglese: il system
//! prompt di un segretario, il messaggio con cui inizia un turno e la richiesta
//! del riassunto alla chiusura della sessione.
//!
//! Al risveglio il contesto si costruisce da zero: riassunto e dati del
//! thread, gli ultimi `RECENT_MESSAGES` messaggi e il messaggio che lo ha
//! svegliato. Nei turni successivi della stessa sessione arrivano solo i
//! messaggi comparsi nel thread dall'ultimo turno (non quelli del segretario,
//! che li ha gia' nella conversazione) e il nuovo messaggio dell'utente.

use super::chat::ChatMessage;
use super::output;
use crate::hub::model::{Author, Message, Thread};
use crate::hub::settings::{Language, Secretary};
use crate::hub::text::clean_text;
use crate::validate::clip_with_ellipsis;

/// Messaggi del thread nel contesto di un turno, prima di quello che lo apre.
pub const RECENT_MESSAGES: usize = 10;
/// Righe tenute del riassunto di un thread.
pub const SUMMARY_LINES_MAX: usize = 10;
/// Caratteri tenuti del riassunto di un thread.
pub const SUMMARY_MAX: usize = 1500;
/// Caratteri tenuti di ogni voce della conversazione nella richiesta del riassunto.
const TRANSCRIPT_ENTRY_KEPT: usize = 800;

/// L'istruzione fissa del motore; `{name}` e' il nome del segretario.
const ENGINE_INSTRUCTIONS: &str = "You are {name}, a secretary of the Agents HUB of cli-companion: an app where \
coding agents work on the tasks of a board, one board per workspace. You talk with the user in a thread of a \
channel. You do not know the code of the projects and you never decide technical questions: report facts, and \
leave technical choices to the user and to the coding agents. Use your tools to read the current state of the \
board and of the thread instead of guessing, and never invent task ids or states. Keep your replies short. Your \
final message, without tool calls, is posted in the thread as your reply.";

const SUMMARY_INSTRUCTIONS: &str = "You keep the memory of a thread of the Agents HUB. From the previous summary and \
the conversation of the session that just ended, write the updated summary of the thread: at most 10 short lines, \
in English, plain text without headings. Keep what will matter later: the goal, the decisions, the task ids with \
their state, the open questions and what the user is waiting for. Reply with the summary only.";

const NEW_MESSAGE: &str = "New message from the user:";

/// System prompt del segretario + istruzione fissa del motore + lingua.
pub fn system_prompt(secretary: &Secretary, language: Language) -> String {
    format!(
        "{}\n\n{}\n\nAlways reply to the user in {}.",
        secretary.system_prompt,
        ENGINE_INSTRUCTIONS.replace("{name}", &secretary.name),
        language.english_name()
    )
}

/// Il messaggio che apre un turno di `me` per il messaggio `trigger_id`.
/// `last_seen` e' il messaggio che ha aperto il turno precedente della
/// sessione (`None` al risveglio, o se e' stato potato). `None` se il
/// messaggio non e' (piu') nel thread.
pub fn turn_message(thread: &Thread, trigger_id: &str, last_seen: Option<&str>, me: &str) -> Option<String> {
    let at = thread.messages.iter().position(|m| m.id == trigger_id)?;
    let before = &thread.messages[..at];
    let seen = last_seen.and_then(|id| before.iter().position(|m| m.id == id));
    let mut out = match seen {
        Some(seen) => since_last_turn(&before[seen + 1..], me),
        None => wake_context(thread, before),
    };
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(NEW_MESSAGE);
    out.push('\n');
    out.push_str(&thread.messages[at].body);
    Some(out)
}

fn wake_context(thread: &Thread, before: &[Message]) -> String {
    let recent = &before[before.len().saturating_sub(RECENT_MESSAGES)..];
    format!(
        "This thread (JSON):\n{}\n\nLatest messages of the thread, oldest first (JSON):\n{}",
        output::header(thread),
        output::messages(recent)
    )
}

fn since_last_turn(new: &[Message], me: &str) -> String {
    let others: Vec<Message> = new.iter().filter(|m| !is_by(m, me)).cloned().collect();
    if others.is_empty() {
        return String::new();
    }
    let recent = &others[others.len().saturating_sub(RECENT_MESSAGES)..];
    format!("Messages posted in the thread since your last reply, oldest first (JSON):\n{}", output::messages(recent))
}

fn is_by(message: &Message, name: &str) -> bool {
    matches!(&message.author, Author::Bot { name: author } if author == name)
}

/// System prompt della richiesta del riassunto.
pub fn summary_system() -> &'static str {
    SUMMARY_INSTRUCTIONS
}

/// Il messaggio della richiesta del riassunto: il riassunto precedente e la
/// conversazione della sessione, con le voci tagliate.
pub fn summary_message(previous: Option<&str>, conversation: &[ChatMessage]) -> String {
    let previous = previous.unwrap_or("(none)");
    let transcript: Vec<String> = conversation.iter().flat_map(transcript_entries).collect();
    format!("Previous summary:\n{previous}\n\nConversation of the session:\n{}", transcript.join("\n"))
}

fn transcript_entries(message: &ChatMessage) -> Vec<String> {
    let clip = |s: &str| clip_with_ellipsis(s, TRANSCRIPT_ENTRY_KEPT);
    match message {
        ChatMessage::User(text) => vec![format!("[user] {}", clip(text))],
        ChatMessage::Assistant { text, tool_calls } => text
            .iter()
            .map(|t| format!("[you] {}", clip(t)))
            .chain(tool_calls.iter().map(|c| format!("[you called {}] {}", c.name, clip(&c.arguments))))
            .collect(),
        ChatMessage::Tool { content, .. } => vec![format!("[tool result] {}", clip(content))],
    }
}

/// Il riassunto scritto dal modello, ripulito: niente caratteri di controllo
/// ne' righe vuote, al massimo `SUMMARY_LINES_MAX` righe e `SUMMARY_MAX`
/// caratteri. `None` se non resta nulla.
pub fn clean_summary(raw: &str) -> Option<String> {
    let cleaned = clean_text(raw, SUMMARY_MAX);
    let lines: Vec<&str> = cleaned.lines().map(str::trim).filter(|l| !l.is_empty()).take(SUMMARY_LINES_MAX).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::chat::ToolCall;
    use crate::hub::engine::fixtures::{bot, numbered, secretary, thread, user};

    #[test]
    fn the_system_prompt_adds_the_engine_rules_and_the_language() {
        let p = system_prompt(&secretary("mario"), Language::De);
        assert!(p.starts_with("You are mario. Report the board status.\n\n"));
        assert!(p.contains("You are mario, a secretary of the Agents HUB"));
        assert!(p.contains("never decide technical questions"));
        assert!(p.ends_with("Always reply to the user in German."));
        assert!(system_prompt(&secretary("mario"), Language::It).ends_with("in Italian."));
    }

    #[test]
    fn a_wake_carries_summary_and_the_last_ten_messages() {
        let mut messages = numbered(15);
        messages.push(user("trigger", "@mario a che punto siamo?"));
        let mut t = thread(messages);
        t.summary = Some("Login: t-1 running.".into());
        let text = turn_message(&t, "trigger", None, "mario").unwrap();
        assert!(text.contains(r#""summary":"Login: t-1 running.""#), "{text}");
        assert!(text.contains(r#""title":"Login""#));
        assert!(!text.contains(r#""id":"m4""#) && text.contains(r#""id":"m5""#) && text.contains(r#""id":"m14""#));
        assert!(!text.contains(r#""id":"trigger""#));
        assert!(text.ends_with("New message from the user:\n@mario a che punto siamo?"));
    }

    #[test]
    fn a_follow_up_carries_only_what_is_new_and_not_by_the_secretary() {
        let t = thread(vec![
            user("m1", "@mario stato?"),
            bot("m2", "mario", "Tutto in coda."),
            bot("m3", "giorgio", "Io controllo i test."),
            user("m4", "e il login?"),
        ]);
        let text = turn_message(&t, "m4", Some("m1"), "mario").unwrap();
        assert!(text.starts_with("Messages posted in the thread since your last reply"));
        assert!(text.contains("Io controllo i test.") && !text.contains("Tutto in coda."));
        assert!(!text.contains("summary") && !text.contains("\"title\""));
        let only_mine = thread(vec![user("m1", "x"), bot("m2", "mario", "y"), user("m3", "e poi?")]);
        assert_eq!(turn_message(&only_mine, "m3", Some("m1"), "mario").unwrap(), "New message from the user:\ne poi?");
    }

    #[test]
    fn a_pruned_last_seen_falls_back_to_a_full_wake_and_a_missing_trigger_to_none() {
        let t = thread(vec![user("m1", "x"), user("m2", "y")]);
        assert!(turn_message(&t, "m2", Some("gone"), "mario").unwrap().starts_with("This thread (JSON)"));
        assert!(turn_message(&t, "gone", None, "mario").is_none());
    }

    #[test]
    fn the_summary_request_has_the_previous_summary_and_the_session() {
        let conversation = vec![
            ChatMessage::User("stato?".into()),
            ChatMessage::Assistant {
                text: None,
                tool_calls: vec![ToolCall { id: "c".into(), name: "board_status".into(), arguments: "{}".into() }],
            },
            ChatMessage::Tool { call_id: "c".into(), content: "x".repeat(TRANSCRIPT_ENTRY_KEPT * 2) },
            ChatMessage::Assistant { text: Some("t-1 e' in corso.".into()), tool_calls: vec![] },
        ];
        let text = summary_message(Some("Old."), &conversation);
        assert!(text.starts_with("Previous summary:\nOld.\n\nConversation of the session:\n[user] stato?\n"));
        assert!(text.contains("[you called board_status] {}") && text.ends_with("[you] t-1 e' in corso."));
        assert!(text.len() < TRANSCRIPT_ENTRY_KEPT * 2);
        assert!(summary_message(None, &[]).contains("(none)"));
    }

    #[test]
    fn summaries_are_cleaned_and_bounded() {
        assert_eq!(clean_summary("  \n line one \n\n\u{7}line two\n").as_deref(), Some("line one\nline two"));
        let many: String = (0..20).map(|i| format!("line {i}\n")).collect();
        assert_eq!(clean_summary(&many).unwrap().lines().count(), SUMMARY_LINES_MAX);
        assert!(clean_summary(&"x".repeat(SUMMARY_MAX * 2)).unwrap().chars().count() <= SUMMARY_MAX + 1);
        assert_eq!(clean_summary("  \n "), None);
    }
}
