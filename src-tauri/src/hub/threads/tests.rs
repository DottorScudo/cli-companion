use super::lifecycle::{CLOSED_THREADS_MAX, PROPOSALS_PER_THREAD_MAX, TASKS_PER_THREAD_MAX};
use super::*;
use crate::hub::attachments::ATTACHMENTS_MAX;
use crate::hub::model::HubEventKind;

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn with_thread(id: &str, now: i64) -> ChannelThreads {
    let mut list = ChannelThreads::default();
    list.create_thread(&ws(), id.to_string(), "Obiettivo", now).unwrap();
    list
}

fn user_text(id: &str, body: &str) -> NewMessage {
    NewMessage {
        id: id.to_string(),
        author: Author::User,
        body: body.to_string(),
        reply_to: None,
        event: None,
        origin: None,
    }
}

fn event(id: &str, kind: HubEventKind, task: &str) -> NewMessage {
    NewMessage {
        id: id.to_string(),
        author: Author::System,
        body: format!("evento {kind:?}"),
        reply_to: None,
        event: Some(HubEvent::of_task(kind, task)),
        origin: None,
    }
}

#[test]
fn a_new_thread_is_open_and_empty() {
    let list = with_thread("t1", 10);
    let t = list.thread("t1").unwrap();
    assert_eq!((t.state, t.title.as_str(), t.created_at, t.updated_at), (ThreadState::Open, "Obiettivo", 10, 10));
    assert_eq!(t.channel, ws());
    assert!(t.messages.is_empty() && t.task_ids.is_empty() && t.proposal_ids.is_empty());
}

#[test]
fn creation_validates_title_and_id() {
    let mut list = with_thread("t1", 0);
    assert!(list.create_thread(&ws(), "t2".to_string(), "  ", 0).is_err());
    assert!(list.create_thread(&ws(), "t2".to_string(), &"x".repeat(121), 0).is_err());
    assert!(list.create_thread(&ws(), "t1".to_string(), "Altro", 0).is_err());
    assert_eq!(list.threads.len(), 1);
}

#[test]
fn messages_are_appended_trimmed_and_touch_the_thread() {
    let mut list = with_thread("t1", 0);
    let m = list.add_message("t1", user_text("m1", "  ciao  "), 5).unwrap();
    assert_eq!((m.body.as_str(), m.kind, m.created_at), ("ciao", MessageKind::Text, 5));
    assert_eq!(m.author, Author::User);
    assert_eq!(list.thread("t1").unwrap().updated_at, 5);
}

#[test]
fn invalid_messages_change_nothing() {
    let mut list = with_thread("t1", 0);
    let before = list.clone();
    assert_eq!(list.add_message("t1", user_text("m1", "   "), 1).unwrap_err(), "il messaggio è vuoto");
    assert!(list.add_message("t1", user_text("m1", &"x".repeat(BODY_MAX + 1)), 1).is_err());
    assert_eq!(list.add_message("nope", user_text("m1", "ciao"), 1).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list, before);
}

#[test]
fn a_reply_must_target_a_message_of_the_same_thread() {
    let mut list = with_thread("t1", 0);
    list.create_thread(&ws(), "t2".to_string(), "Altro", 0).unwrap();
    list.add_message("t2", user_text("other", "nell'altro thread"), 1).unwrap();
    list.add_message("t1", user_text("m1", "primo"), 1).unwrap();
    let reply = NewMessage { reply_to: Some("m1".to_string()), ..user_text("m2", "risposta") };
    assert_eq!(list.add_message("t1", reply, 2).unwrap().reply_to.as_deref(), Some("m1"));
    for target in ["missing", "other"] {
        let reply = NewMessage { reply_to: Some(target.to_string()), ..user_text("m3", "x") };
        assert_eq!(list.add_message("t1", reply, 3).unwrap_err(), "il messaggio a cui rispondi non è in questo thread");
    }
}

#[test]
fn duplicate_message_ids_are_rejected() {
    let mut list = with_thread("t1", 0);
    list.add_message("t1", user_text("m1", "a"), 1).unwrap();
    assert!(list.add_message("t1", user_text("m1", "b"), 2).is_err());
}

#[test]
fn event_messages_carry_their_data() {
    let mut list = with_thread("t1", 0);
    let m = list.add_message("t1", event("e1", HubEventKind::TaskFailed, "api-fix"), 1).unwrap();
    assert_eq!(m.kind, MessageKind::Event);
    assert_eq!(m.event.as_ref().unwrap().task_id.as_deref(), Some("api-fix"));
    assert!(list.summaries()[0].waiting_user);
}

#[test]
fn events_are_validated() {
    let mut list = with_thread("t1", 0);
    let mut no_subject = event("e1", HubEventKind::ProposalCreated, "a");
    assert!(list.add_message("t1", no_subject.clone(), 1).is_err());
    no_subject.event.as_mut().unwrap().proposal_id = Some("../p".to_string());
    assert!(list.add_message("t1", no_subject, 1).is_err());
    assert!(list.add_message("t1", event("e2", HubEventKind::TaskDone, "Bad Id"), 1).is_err());
    let mut long = event("e3", HubEventKind::TaskDone, "a");
    long.event.as_mut().unwrap().summary = Some("x".repeat(BODY_MAX + 1));
    assert!(list.add_message("t1", long, 1).is_err());
    let mut long_note = event("e4", HubEventKind::TaskFailed, "a");
    long_note.event.as_mut().unwrap().note = Some("x".repeat(BODY_MAX + 1));
    assert!(list.add_message("t1", long_note, 1).is_err());
    for bad_ws in ["general", "../x"] {
        let mut foreign = event("e5", HubEventKind::TaskDone, "a");
        foreign.event.as_mut().unwrap().workspace = Some(bad_ws.to_string());
        assert!(list.add_message("t1", foreign, 1).is_err(), "{bad_ws}");
    }
    assert!(list.thread("t1").unwrap().messages.is_empty());
}

#[test]
fn question_events_are_validated() {
    use crate::board::{OPTIONS_MAX, OPTION_MAX, QUESTION_MAX};
    use crate::hub::model::HubQuestion;
    let mut list = with_thread("t1", 0);
    let with_question = |id: &str, question: HubQuestion| {
        let mut m = event(id, HubEventKind::TaskQuestion, "a");
        m.event.as_mut().unwrap().question = Some(question);
        m
    };
    let good = HubQuestion { id: "q-1-1".into(), text: "Quale?".into(), options: vec!["A".into(), "B".into()] };
    let added = list.add_message("t1", with_question("e1", good.clone()), 1).unwrap();
    assert_eq!(added.event.as_ref().unwrap().question.as_ref(), Some(&good));
    let bad = [
        HubQuestion { id: "../q".into(), ..good.clone() },
        HubQuestion { text: "x".repeat(QUESTION_MAX + 1), ..good.clone() },
        HubQuestion { options: vec!["o".into(); OPTIONS_MAX + 1], ..good.clone() },
        HubQuestion { options: vec!["o".repeat(OPTION_MAX + 1)], ..good.clone() },
    ];
    for (i, question) in bad.into_iter().enumerate() {
        assert!(list.add_message("t1", with_question(&format!("e{}", i + 2), question), 1).is_err(), "{i}");
    }
    assert_eq!(list.thread("t1").unwrap().messages.len(), 1);
}

#[test]
fn bot_authors_are_validated() {
    let mut list = with_thread("t1", 0);
    let bot = NewMessage { author: Author::Bot { name: " mario ".to_string() }, ..user_text("m1", "ciao") };
    assert_eq!(list.add_message("t1", bot, 1).unwrap().author, Author::Bot { name: "mario".to_string() });
    let nameless = NewMessage { author: Author::Bot { name: String::new() }, ..user_text("m2", "ciao") };
    assert!(list.add_message("t1", nameless, 1).is_err());
}

#[test]
fn the_oldest_messages_are_pruned_past_the_limit() {
    let mut list = with_thread("t1", 0);
    for i in 0..MESSAGES_MAX + 3 {
        list.add_message("t1", user_text(&format!("m{i}"), "x"), i as i64).unwrap();
    }
    let msgs = &list.thread("t1").unwrap().messages;
    assert_eq!(msgs.len(), MESSAGES_MAX);
    assert_eq!(msgs[0].id, "m3");
    assert_eq!(msgs.last().unwrap().id, format!("m{}", MESSAGES_MAX + 2));
}

#[test]
fn threads_close_and_reopen() {
    let mut list = with_thread("t1", 0);
    assert_eq!(list.set_state("t1", ThreadState::Closed, 4).unwrap().state, ThreadState::Closed);
    assert_eq!(list.thread("t1").unwrap().updated_at, 4);
    // Gia' chiuso: nessun cambiamento.
    assert_eq!(list.set_state("t1", ThreadState::Closed, 9).unwrap().updated_at, 4);
    assert_eq!(list.set_state("t1", ThreadState::Open, 6).unwrap().state, ThreadState::Open);
    assert_eq!(list.set_state("nope", ThreadState::Closed, 7).unwrap_err(), THREAD_NOT_FOUND);
}

#[test]
fn a_closed_thread_still_accepts_messages() {
    let mut list = with_thread("t1", 0);
    list.set_state("t1", ThreadState::Closed, 1).unwrap();
    list.add_message("t1", event("e1", HubEventKind::TaskDone, "a"), 2).unwrap();
    assert_eq!(list.thread("t1").unwrap().state, ThreadState::Closed);
}

#[test]
fn the_oldest_closed_threads_are_pruned_past_the_limit() {
    let mut list = ChannelThreads::default();
    list.create_thread(&ws(), "open".to_string(), "Aperto", 0).unwrap();
    for i in 0..CLOSED_THREADS_MAX + 2 {
        let id = format!("c{i}");
        list.create_thread(&ws(), id.clone(), "Chiuso", 0).unwrap();
        list.set_state(&id, ThreadState::Closed, 10 + i as i64).unwrap();
    }
    let closed: Vec<&str> =
        list.threads.iter().filter(|t| t.state == ThreadState::Closed).map(|t| t.id.as_str()).collect();
    assert_eq!(closed.len(), CLOSED_THREADS_MAX);
    assert!(!closed.contains(&"c0") && !closed.contains(&"c1") && closed.contains(&"c2"));
    assert!(list.thread("open").is_some(), "gli aperti non si potano");
}

#[test]
fn the_thread_just_closed_is_never_pruned() {
    let mut list = ChannelThreads::default();
    for i in 0..CLOSED_THREADS_MAX {
        let id = format!("c{i}");
        list.create_thread(&ws(), id.clone(), "Chiuso", 0).unwrap();
        list.set_state(&id, ThreadState::Closed, 100).unwrap();
    }
    // Aggiornato prima di tutti gli altri, ma e' quello che si sta chiudendo.
    list.create_thread(&ws(), "late".to_string(), "Ultimo", 0).unwrap();
    list.threads.last_mut().unwrap().updated_at = 1;
    list.threads.last_mut().unwrap().state = ThreadState::Closed;
    list.set_state("late", ThreadState::Closed, 1).unwrap();
    assert!(list.thread("late").is_some());
    assert_eq!(list.threads.len(), CLOSED_THREADS_MAX);
}

#[test]
fn tasks_are_linked_once_and_found() {
    let mut list = with_thread("t1", 0);
    list.create_thread(&ws(), "t2".to_string(), "Altro", 0).unwrap();
    let ids = vec!["api".to_string(), "ui".to_string(), "api".to_string()];
    assert_eq!(list.link_tasks("t1", &ids, 3).unwrap().task_ids, ["api", "ui"]);
    assert_eq!(list.link_tasks("t1", &["ui".to_string()], 8).unwrap().updated_at, 3, "nessun cambiamento");
    assert_eq!(list.thread_of_task("ui").unwrap().id, "t1");
    assert!(list.thread_of_task("db").is_none());
    let err = list.link_tasks("t2", &["db".to_string(), "api".to_string()], 4).unwrap_err();
    assert_eq!(err, "il task api è già collegato a un altro thread");
    assert!(list.thread("t2").unwrap().task_ids.is_empty(), "tutto o niente");
    assert!(list.link_tasks("t1", &["Not Valid".to_string()], 4).is_err());
    assert_eq!(list.link_tasks("nope", &[], 4).unwrap_err(), THREAD_NOT_FOUND);
}

#[test]
fn linked_tasks_are_bounded() {
    let mut list = with_thread("t1", 0);
    let ids: Vec<String> = (0..=TASKS_PER_THREAD_MAX).map(|i| format!("t{i}")).collect();
    assert!(list.link_tasks("t1", &ids, 1).is_err());
    assert!(list.link_tasks("t1", &ids[..TASKS_PER_THREAD_MAX], 1).is_ok());
}

#[test]
fn proposals_are_linked_and_found() {
    let mut list = with_thread("t1", 0);
    list.create_thread(&ws(), "t2".to_string(), "Altro", 0).unwrap();
    list.link_proposal("t1", "p-1-1", 2).unwrap();
    assert_eq!(list.thread_of_proposal("p-1-1").unwrap().id, "t1");
    assert!(list.thread_of_proposal("p-9").is_none());
    assert!(list.link_proposal("t2", "p-1-1", 3).is_err());
    assert!(list.link_proposal("t1", "p 1", 3).is_err());
    list.link_proposal("t1", "p-2-1", 4).unwrap();
    list.link_proposal("t1", "p-2-1", 5).unwrap();
    let t = list.thread("t1").unwrap();
    assert_eq!((t.proposal_ids.as_slice(), t.updated_at), (&["p-1-1".to_string(), "p-2-1".to_string()][..], 4));
}

#[test]
fn a_decided_proposal_is_unlinked_without_touching_the_thread() {
    let mut list = with_thread("t1", 0);
    list.link_proposal("t1", "p-1", 2).unwrap();
    list.link_proposal("t1", "p-2", 3).unwrap();
    list.unlink_proposal("p-1");
    list.unlink_proposal("p-9");
    let t = list.thread("t1").unwrap();
    assert_eq!((t.proposal_ids.as_slice(), t.updated_at), (&["p-2".to_string()][..], 3));
    assert!(list.thread_of_proposal("p-1").is_none());
}

#[test]
fn linked_proposals_are_bounded_dropping_the_oldest() {
    let mut list = with_thread("t1", 0);
    for i in 0..=PROPOSALS_PER_THREAD_MAX {
        list.link_proposal("t1", &format!("p-{i}"), 1).unwrap();
    }
    let ids = &list.thread("t1").unwrap().proposal_ids;
    assert_eq!(ids.len(), PROPOSALS_PER_THREAD_MAX);
    assert_eq!(
        (ids[0].as_str(), ids.last().map(String::as_str)),
        ("p-1", Some(&*format!("p-{PROPOSALS_PER_THREAD_MAX}")))
    );
}

#[test]
fn a_file_with_the_old_single_proposal_keeps_its_link() {
    let thread = |proposal: &str| {
        format!(
            r#"{{"threads":[{{"id":"t1","channel":"ws-1","title":"T","state":"open","created_at":1,"updated_at":2{proposal}}}]}}"#
        )
    };
    let linked: ChannelThreads = serde_json::from_str(&thread(r#","proposal_id":"p-1""#)).unwrap();
    assert_eq!(linked.thread_of_proposal("p-1").unwrap().proposal_ids, ["p-1"]);
    let unlinked: ChannelThreads = serde_json::from_str(&thread(r#","proposal_id":null"#)).unwrap();
    assert!(unlinked.thread("t1").unwrap().proposal_ids.is_empty());

    let saved = serde_json::to_string(&linked).unwrap();
    assert!(saved.contains(r#""proposal_ids":["p-1"]"#) && !saved.contains(r#""proposal_id""#), "{saved}");
    assert_eq!(serde_json::from_str::<ChannelThreads>(&saved).unwrap(), linked);
}

#[test]
fn summaries_are_newest_first_without_messages() {
    let mut list = with_thread("old", 1);
    list.create_thread(&ws(), "new".to_string(), "Nuovo", 2).unwrap();
    list.add_message("old", user_text("m1", "ciao"), 5).unwrap();
    list.link_tasks("old", &["a".to_string()], 6).unwrap();
    let s = list.summaries();
    assert_eq!(s.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["old", "new"]);
    assert_eq!((s[0].message_count, s[0].task_count, s[0].updated_at, s[0].waiting_user), (1, 1, 6, false));
}

#[test]
fn threads_of_another_channel_are_dropped() {
    let mut list = with_thread("t1", 0);
    list.create_thread(&Channel::General, "g1".to_string(), "Generale", 0).unwrap();
    assert_eq!(list.retain_channel(&ws()), 1);
    assert!(list.thread("g1").is_none() && list.thread("t1").is_some());
}

#[test]
fn serde_roundtrip_keeps_everything() {
    let mut list = with_thread("t1", 0);
    list.add_message("t1", user_text("m1", "ciao"), 1).unwrap();
    list.add_message("t1", event("e1", HubEventKind::TaskAttention, "api"), 2).unwrap();
    let bot = NewMessage {
        author: Author::Bot { name: "mario".to_string() },
        reply_to: Some("m1".to_string()),
        ..user_text("m2", "risposta")
    };
    list.add_message("t1", bot, 3).unwrap();
    list.link_tasks("t1", &["api".to_string()], 4).unwrap();
    list.link_proposal("t1", "p-1-1", 5).unwrap();
    let json = serde_json::to_string(&list).unwrap();
    assert_eq!(serde_json::from_str::<ChannelThreads>(&json).unwrap(), list);
    assert!(json.contains(r#""author":{"type":"bot","name":"mario"}"#), "{json}");
    assert!(json.contains(r#""channel":"ws-1""#) && json.contains(r#""kind":"task_attention""#), "{json}");
}

#[test]
fn a_file_without_optional_fields_is_read() {
    let json = r#"{"threads":[{"id":"t1","channel":"general","title":"T","state":"open","created_at":1,"updated_at":2,
        "messages":[{"id":"m1","author":{"type":"user"},"kind":"text","body":"ciao","created_at":1}]}]}"#;
    let list: ChannelThreads = serde_json::from_str(json).unwrap();
    let t = list.thread("t1").unwrap();
    assert_eq!(t.channel, Channel::General);
    assert!(t.task_ids.is_empty() && t.proposal_ids.is_empty());
    assert!(t.messages[0].reply_to.is_none() && t.messages[0].event.is_none());
    assert_eq!(serde_json::from_str::<ChannelThreads>("{}").unwrap(), ChannelThreads::default());
}

#[test]
fn an_event_written_before_the_whole_lengths_is_read_without_them() {
    let json = r#"{"threads":[{"id":"t1","channel":"ws-1","title":"T","state":"open","created_at":1,"updated_at":2,
        "messages":[{"id":"m1","author":{"type":"system"},"kind":"event","body":"Task fallito: X","created_at":1,
        "event":{"kind":"task_failed","workspace":"ws-1","task_id":"x","proposal_id":null,"status":"failed",
        "summary":"riassunto…","verdict":null,"note":"nota","question":null,"request":null}}]}]}"#;
    let list: ChannelThreads = serde_json::from_str(json).unwrap();
    let event = list.thread("t1").unwrap().messages[0].event.as_ref().unwrap();
    assert_eq!((event.summary.as_deref(), event.summary_chars, event.note_chars), (Some("riassunto…"), None, None));
    let saved = serde_json::to_string(&list).unwrap();
    assert!(!saved.contains("summary_chars") && !saved.contains("note_chars"), "{saved}");
}

#[test]
fn the_summary_is_replaced_without_touching_the_thread() {
    let mut list = with_thread("t1", 3);
    assert!(list.thread("t1").unwrap().summary.is_none());
    assert!(list.set_summary("t1", "Il task t-1 è in corso.".to_string()).unwrap());
    assert!(!list.set_summary("t1", "Il task t-1 è in corso.".to_string()).unwrap());
    let t = list.thread("t1").unwrap();
    assert_eq!((t.summary.as_deref(), t.updated_at), (Some("Il task t-1 è in corso."), 3));
    assert_eq!(list.set_summary("nope", "x".to_string()).unwrap_err(), THREAD_NOT_FOUND);
}

#[test]
fn threads_written_before_the_summary_read_without_it() {
    let raw = r#"{"threads":[{"id":"t1","channel":"ws-1","title":"x","state":"open","created_at":1,"updated_at":1}]}"#;
    let list: ChannelThreads = serde_json::from_str(raw).unwrap();
    assert!(list.thread("t1").unwrap().summary.is_none());
}

#[test]
fn the_summary_carries_the_owner_once_set() {
    let mut list = with_thread("t1", 1);
    assert!(list.summaries()[0].owner.is_none());
    list.set_owner("t1", "giorgio", 2).unwrap();
    assert_eq!(list.summaries()[0].owner.as_deref(), Some("giorgio"));
}

#[test]
fn the_owner_is_set_replaced_and_validated() {
    let mut list = with_thread("t1", 1);
    assert!(list.thread("t1").unwrap().owner.is_none());
    assert_eq!(list.set_owner("t1", "giorgio", 2).unwrap().owner.as_deref(), Some("giorgio"));
    let t = list.set_owner("t1", "mario", 3).unwrap();
    assert_eq!((t.owner.as_deref(), t.updated_at), (Some("mario"), 3));
    assert_eq!(list.set_owner("t1", "mario", 9).unwrap().updated_at, 3, "nessun cambiamento");
    assert!(list.set_owner("t1", "   ", 4).is_err());
    assert!(list.set_owner("t1", &"a".repeat(40), 4).is_err());
    assert_eq!(list.set_owner("nope", "mario", 4).unwrap_err(), THREAD_NOT_FOUND);
    // Un file scritto prima del titolare si legge senza.
    let raw = r#"{"threads":[{"id":"t1","channel":"ws-1","title":"x","state":"open","created_at":1,"updated_at":1}]}"#;
    assert!(serde_json::from_str::<ChannelThreads>(raw).unwrap().thread("t1").unwrap().owner.is_none());
}

fn attachment(id: &str) -> Attachment {
    Attachment { id: id.to_string(), name: "foto.png".to_string(), mime: "image/png".to_string(), size: 10 }
}

#[test]
fn a_user_message_with_attachments_may_have_no_text() {
    let mut list = with_thread("t1", 1);
    let m = list.add_message_with_attachments("t1", user_text("m1", "  "), vec![attachment("att-1-1")], 2).unwrap();
    assert_eq!((m.body.as_str(), m.attachments.len()), ("", 1));
    assert_eq!(list.attachment("t1", "att-1-1").unwrap().name, "foto.png");
    assert_eq!(
        list.add_message_with_attachments("t1", user_text("m2", " "), vec![], 3).unwrap_err(),
        "il messaggio è vuoto"
    );
    assert!(list.add_message("t1", user_text("m3", "solo testo"), 4).unwrap().attachments.is_empty());
}

#[test]
fn only_the_user_attaches_files_within_the_limits() {
    let mut list = with_thread("t1", 1);
    let bot = NewMessage { author: Author::Bot { name: "mario".to_string() }, ..user_text("m1", "ciao") };
    assert!(list
        .add_message_with_attachments("t1", bot, vec![attachment("att-1-1")], 2)
        .unwrap_err()
        .contains("utente"));
    let too_many: Vec<Attachment> = (0..=ATTACHMENTS_MAX).map(|i| attachment(&format!("att-1-{i}"))).collect();
    assert!(list.add_message_with_attachments("t1", user_text("m2", "x"), too_many, 2).is_err());
    let bad_name = Attachment { name: "../x".to_string(), ..attachment("att-1-1") };
    assert!(list.add_message_with_attachments("t1", user_text("m3", "x"), vec![bad_name], 2).is_err());
    assert!(list.thread("t1").unwrap().messages.is_empty());
}

#[test]
fn looking_up_an_attachment_needs_its_thread() {
    let mut list = with_thread("t1", 1);
    list.add_message_with_attachments("t1", user_text("m1", ""), vec![attachment("att-1-1")], 2).unwrap();
    assert_eq!(list.attachment("t2", "att-1-1").unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.attachment("t1", "att-1-2").unwrap_err(), attachments::NOT_FOUND);
}
