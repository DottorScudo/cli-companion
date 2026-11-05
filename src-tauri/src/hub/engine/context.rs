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
use super::sessions::WakeCause;
use super::workspace_names::WorkspaceNames;
use crate::hub::attachments::listing;
use crate::hub::channel::Channel;
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
board and of the thread instead of guessing, and never invent task ids or states. Always refer to a workspace by its name and never show a workspace id to the \
user (task ids are fine: the app shows them); when the user names a project, match it to the workspace names you \
are given. Keep your replies short. Your \
final message, without tool calls, is posted in the thread as your reply.";

const SUMMARY_INSTRUCTIONS: &str = "You keep the memory of a thread of the Agents HUB. From the previous summary and \
the conversation of the session that just ended, write the updated summary of the thread: at most 10 short lines, \
in English, plain text without headings. Keep what will matter later: the goal, the decisions, the task ids with \
their state, the open questions and what the user is waiting for. Reply with the summary only.";

const NEW_MESSAGE: &str = "New message from the user:";
/// Dopo l'elenco degli allegati del messaggio del turno: i segretari non li aprono.
const ATTACHMENTS_NOTE: &str = "You cannot open these files and you never see their content or location: you only \
know what is listed here. Mention them to the user if it helps and, if the request needs the files, hand it over to \
planning: the planner can read them.";
const BOARD_EVENT: &str = "A board event in this thread woke you up (JSON):";

/// Cosa fare quando un evento della board sveglia il titolare del thread.
const EVENT_GUIDE: &str = "You are the owner of this thread because you opened its planning task. Tell the user in \
a few lines what happened and what, if anything, they need to do. A new proposal: present its tasks briefly and ask \
the user to use Reply on the proposal message to accept or reject it; you cannot decide it yourself now. A plan that \
ended without a proposal: its summary holds the planner's questions, ask them to the user; once they answer you can \
open a new planning task with the original goal, the questions and the answers. A question from a worker or from \
the planner: present it in a few words, with its options if any, help the user understand it if needed, and tell \
them to use Reply on the question message: their reply reaches the worker as written. You cannot answer it yourself \
now. A failed task or a task that needs attention: say which one and why, if known. All tasks concluded: sum up the \
outcome.";

/// Cosa ricordare quando il messaggio dell'utente risponde a una proposta pendente.
const PROPOSAL_REPLY: &str = "This message is the user's reply to the pending task proposal {proposal}. If the user \
clearly accepts or rejects it, call decide_proposal; if the intent is not clear, ask the user instead of deciding.";

/// Il messaggio che apre un turno e perche' lo apre.
pub struct Trigger<'a> {
    pub id: &'a str,
    pub cause: WakeCause,
    /// La proposta pendente a cui risponde il messaggio dell'utente
    /// (`planning::proposal_target`), se c'e'.
    pub proposal: Option<&'a str>,
}

/// System prompt del segretario + istruzione fissa del motore + lingua.
pub fn system_prompt(secretary: &Secretary, language: Language, channel: &Channel, names: &WorkspaceNames) -> String {
    format!(
        "{}\n\n{}\n\n{}\n\nAlways reply to the user in {}.",
        secretary.system_prompt,
        ENGINE_INSTRUCTIONS.replace("{name}", &secretary.name),
        names.describe(channel, secretary),
        language.english_name()
    )
}

/// Il messaggio che apre un turno di `me` per il messaggio `trigger`: di
/// un utente o, per un risveglio da evento, l'evento della board con le
/// indicazioni per il titolare. `last_seen` e' il messaggio che ha aperto il
/// turno precedente della sessione (`None` al risveglio, o se e' stato
/// potato). `None` se il messaggio non e' (piu') nel thread.
pub fn turn_message(
    thread: &Thread,
    trigger: &Trigger<'_>,
    last_seen: Option<&str>,
    me: &str,
    names: &WorkspaceNames,
) -> Option<String> {
    let at = thread.messages.iter().position(|m| m.id == trigger.id)?;
    let before = &thread.messages[..at];
    let seen = last_seen.and_then(|id| before.iter().position(|m| m.id == id));
    let mut out = match seen {
        Some(seen) => since_last_turn(&before[seen + 1..], me, names),
        None => wake_context(thread, before, names),
    };
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    let message = &thread.messages[at];
    match trigger.cause {
        WakeCause::Message => {
            out.push_str(NEW_MESSAGE);
            out.push('\n');
            out.push_str(&message.body);
            push_attachments(&mut out, message);
            if let Some(proposal) = trigger.proposal {
                out.push_str("\n\n");
                out.push_str(&PROPOSAL_REPLY.replace("{proposal}", proposal));
            }
        }
        WakeCause::BoardEvent => {
            out.push_str(BOARD_EVENT);
            out.push('\n');
            out.push_str(&output::message(message, names).to_string());
            out.push_str("\n\n");
            out.push_str(EVENT_GUIDE);
        }
    }
    Some(out)
}

fn push_attachments(out: &mut String, message: &Message) {
    if message.attachments.is_empty() {
        return;
    }
    out.push_str("\n\nAttachments:\n");
    for attachment in &message.attachments {
        out.push_str("- ");
        out.push_str(&listing::describe(attachment));
        out.push('\n');
    }
    out.push_str(ATTACHMENTS_NOTE);
}

fn wake_context(thread: &Thread, before: &[Message], names: &WorkspaceNames) -> String {
    let recent = &before[before.len().saturating_sub(RECENT_MESSAGES)..];
    format!(
        "This thread (JSON):\n{}\n\nLatest messages of the thread, oldest first (JSON):\n{}",
        output::header(thread, names),
        output::messages(recent, names)
    )
}

fn since_last_turn(new: &[Message], me: &str, names: &WorkspaceNames) -> String {
    let others: Vec<Message> = new.iter().filter(|m| !is_by(m, me)).cloned().collect();
    if others.is_empty() {
        return String::new();
    }
    let recent = &others[others.len().saturating_sub(RECENT_MESSAGES)..];
    format!(
        "Messages posted in the thread since your last reply, oldest first (JSON):\n{}",
        output::messages(recent, names)
    )
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
    use crate::hub::engine::fixtures::{bot, msg, names, numbered, secretary, thread, user, ws};
    use crate::hub::model::{Attachment, HubEvent, HubEventKind};

    fn by_user(id: &str) -> Trigger<'_> {
        Trigger { id, cause: WakeCause::Message, proposal: None }
    }

    #[test]
    fn the_system_prompt_adds_the_engine_rules_and_the_language() {
        let p = system_prompt(&secretary("mario"), Language::De, &Channel::General, &names());
        assert!(p.starts_with("You are mario. Report the board status.\n\n"));
        assert!(p.contains("You are mario, a secretary of the Agents HUB"));
        assert!(p.contains("never decide technical questions"));
        assert!(p.ends_with("Always reply to the user in German."));
        assert!(system_prompt(&secretary("mario"), Language::It, &ws(), &names()).ends_with("in Italian."));
    }

    #[test]
    fn the_system_prompt_tells_the_channel_and_the_workspaces_by_name() {
        let s = secretary("mario");
        let general = system_prompt(&s, Language::It, &Channel::General, &names());
        assert!(general.contains("never show a workspace id to the user"), "{general}");
        assert!(general.contains("This thread is in the #Generale channel"), "{general}");
        assert!(general.contains("\"Muse Companion\" (id: ws-1)") && general.contains("\"Sito Web\" (id: ws-2)"));
        let workspace = system_prompt(&s, Language::It, &Channel::Workspace("ws-2".into()), &names());
        assert!(workspace.contains("the channel of workspace \"Sito Web\""), "{workspace}");
        assert!(workspace.contains("\"Muse Companion\" (id: ws-1)"), "{workspace}");
    }

    #[test]
    fn a_wake_carries_summary_and_the_last_ten_messages() {
        let mut messages = numbered(15);
        messages.push(user("trigger", "@mario a che punto siamo?"));
        let mut t = thread(messages);
        t.summary = Some("Login: t-1 running.".into());
        let text = turn_message(&t, &by_user("trigger"), None, "mario", &names()).unwrap();
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
        let text = turn_message(&t, &by_user("m4"), Some("m1"), "mario", &names()).unwrap();
        assert!(text.starts_with("Messages posted in the thread since your last reply"));
        assert!(text.contains("Io controllo i test.") && !text.contains("Tutto in coda."));
        assert!(!text.contains("summary") && !text.contains("\"title\""));
        let only_mine = thread(vec![user("m1", "x"), bot("m2", "mario", "y"), user("m3", "e poi?")]);
        assert_eq!(
            turn_message(&only_mine, &by_user("m3"), Some("m1"), "mario", &names()).unwrap(),
            "New message from the user:\ne poi?"
        );
    }

    #[test]
    fn a_pruned_last_seen_falls_back_to_a_full_wake_and_a_missing_trigger_to_none() {
        let t = thread(vec![user("m1", "x"), user("m2", "y")]);
        assert!(turn_message(&t, &by_user("m2"), Some("gone"), "mario", &names())
            .unwrap()
            .starts_with("This thread (JSON)"));
        assert!(turn_message(&t, &by_user("gone"), None, "mario", &names()).is_none());
    }

    #[test]
    fn a_reply_to_a_pending_proposal_is_pointed_out() {
        let t = thread(vec![user("m1", "sì, accetta")]);
        let trigger = Trigger { id: "m1", cause: WakeCause::Message, proposal: Some("p-1") };
        let text = turn_message(&t, &trigger, None, "giorgio", &names()).unwrap();
        assert!(text.contains("New message from the user:\nsì, accetta\n\n"), "{text}");
        assert!(text.ends_with(
            "pending task proposal p-1. If the user clearly accepts or rejects it, call \
decide_proposal; if the intent is not clear, ask the user instead of deciding."
        ));
    }

    #[test]
    fn a_board_event_opens_the_turn_with_its_data_and_the_owner_guide() {
        let mut event = msg("e1", Author::System, "Nuova proposta: 1 task\n- Login", 5);
        event.event = Some(HubEvent {
            workspace: Some("ws-1".into()),
            ..HubEvent::of_proposal(HubEventKind::ProposalCreated, "p-1")
        });
        let t = thread(vec![user("m1", "@giorgio aggiungi il login"), event]);
        let trigger = Trigger { id: "e1", cause: WakeCause::BoardEvent, proposal: None };
        let text = turn_message(&t, &trigger, None, "giorgio", &names()).unwrap();
        assert!(text.starts_with("This thread (JSON)") && text.contains("aggiungi il login"), "{text}");
        assert!(text.contains("A board event in this thread woke you up (JSON):\n{"), "{text}");
        assert!(text.contains(r#""kind":"proposal_created""#) && text.contains(r#""proposal_id":"p-1""#), "{text}");
        assert!(
            text.contains(r#""workspace":"ws-1""#) && text.contains(r#""workspace_name":"Muse Companion""#),
            "{text}"
        );
        assert!(text.contains("you cannot decide it yourself now") && !text.contains("New message from the user"));
    }

    #[test]
    fn a_worker_question_wakes_the_owner_to_present_it_without_answering() {
        let mut event = msg("e1", Author::System, "Domanda del worker: Login\nQuale database?", 5);
        event.event = Some(HubEvent {
            workspace: Some("ws-1".into()),
            question: Some(crate::hub::model::HubQuestion {
                id: "q-1-1".into(),
                text: "Quale database?".into(),
                options: vec!["Postgres".into()],
            }),
            ..HubEvent::of_task(HubEventKind::TaskQuestion, "t-1")
        });
        let t = thread(vec![user("m1", "@giorgio aggiungi il login"), event]);
        let trigger = Trigger { id: "e1", cause: WakeCause::BoardEvent, proposal: None };
        let text = turn_message(&t, &trigger, None, "giorgio", &names()).unwrap();
        assert!(text.contains(r#""kind":"task_question""#) && text.contains(r#""id":"q-1-1""#), "{text}");
        assert!(text.contains(r#""options":["Postgres"]"#), "{text}");
        assert!(text.contains("A question from a worker or from the planner") && text.contains("cannot answer it"));
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

    fn with_attachments(mut message: Message) -> Message {
        message.attachments =
            vec![Attachment { id: "att-1-1".into(), name: "errore.png".into(), mime: "image/png".into(), size: 2048 }];
        message
    }

    #[test]
    fn the_attachments_of_the_turn_message_are_listed_without_path_or_content() {
        let t = thread(vec![with_attachments(user("m1", "guarda questo"))]);
        let text = turn_message(&t, &by_user("m1"), None, "mario", &names()).unwrap();
        assert!(text.contains("New message from the user:\nguarda questo\n\nAttachments:\n"), "{text}");
        assert!(text.contains("- errore.png (image/png, 2.0 KB)\n"), "{text}");
        assert!(text.ends_with("the planner can read them."), "{text}");
        assert!(!text.contains("att-1-1") && !text.contains("hub_attachments"), "{text}");
    }

    #[test]
    fn earlier_messages_of_the_context_carry_their_attachments_as_metadata() {
        let t = thread(vec![with_attachments(user("m1", "ecco")), user("m2", "e quindi?")]);
        let wake = turn_message(&t, &by_user("m2"), None, "mario", &names()).unwrap();
        assert!(wake.contains(r#""attachments":[{"name":"errore.png","size":"2.0 KB","type":"image/png"}]"#), "{wake}");
        let t = thread(vec![user("m0", "x"), with_attachments(user("m1", "ecco")), user("m2", "e quindi?")]);
        let follow_up = turn_message(&t, &by_user("m2"), Some("m0"), "mario", &names()).unwrap();
        assert!(follow_up.contains(r#""name":"errore.png""#) && !follow_up.contains("att-1-1"), "{follow_up}");
    }

    #[test]
    fn a_message_without_attachments_has_no_attachments_block() {
        let t = thread(vec![user("m1", "ciao")]);
        let text = turn_message(&t, &by_user("m1"), None, "mario", &names()).unwrap();
        assert!(!text.contains("Attachments"), "{text}");
    }
}
