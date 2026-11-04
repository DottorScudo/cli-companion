use super::*;
use crate::hub::engine::fixtures::{bot, numbered, proposal_event, reply_to, thread, user, ws, FakeBoard, WS};
use crate::hub::model::HubEvent;
use crate::hub::threads::NewMessage;
use std::cell::RefCell;

fn target(proposal_id: &str) -> ProposalRef {
    ProposalRef { workspace: WS.to_string(), proposal_id: proposal_id.to_string() }
}

/// Il thread con la proposta `p-1` pendente e la risposta `r1` dell'utente.
fn replied(body: &str) -> Thread {
    thread(vec![user("m1", "@giorgio aggiungi il login"), proposal_event("e1", "p-1"), reply_to("r1", "e1", body)])
}

// --- La proposta che un turno puo' decidere ---

#[test]
fn a_user_reply_to_a_pending_proposal_event_is_the_target() {
    assert_eq!(proposal_target(&replied("sì, accetta"), "r1", WakeCause::Message), Some(target("p-1")));
}

#[test]
fn a_board_event_wake_never_has_a_target() {
    let t = replied("sì, accetta");
    assert_eq!(proposal_target(&t, "r1", WakeCause::BoardEvent), None);
    assert_eq!(proposal_target(&t, "e1", WakeCause::BoardEvent), None);
}

#[test]
fn without_a_reply_to_the_proposal_there_is_no_target() {
    // Nessun reply_to.
    let plain = thread(vec![proposal_event("e1", "p-1"), user("m2", "sì, accetta")]);
    assert_eq!(proposal_target(&plain, "m2", WakeCause::Message), None);
    // Risposta a un messaggio che non e' una proposta.
    let other = thread(vec![user("m1", "aggiungi il login"), proposal_event("e1", "p-1"), reply_to("r1", "m1", "sì")]);
    assert_eq!(proposal_target(&other, "r1", WakeCause::Message), None);
    // Risposta a un evento di un task.
    let mut failed = proposal_event("e2", "p-1");
    failed.event = Some(HubEvent::of_task(HubEventKind::TaskFailed, "t-1"));
    let task_reply = thread(vec![failed, reply_to("r1", "e2", "riprova")]);
    assert_eq!(proposal_target(&task_reply, "r1", WakeCause::Message), None);
    // Il messaggio del turno non e' nel thread, o non e' dell'utente.
    assert_eq!(proposal_target(&replied("sì"), "gone", WakeCause::Message), None);
    let mut by_bot = replied("sì");
    by_bot.messages[2].author = Author::Bot { name: "giorgio".into() };
    assert_eq!(proposal_target(&by_bot, "r1", WakeCause::Message), None);
}

#[test]
fn a_proposal_already_decided_or_of_another_workspace_is_no_target() {
    let mut decided = replied("sì");
    let mut accepted = proposal_event("e9", "p-1");
    accepted.event =
        Some(HubEvent { workspace: Some(WS.into()), ..HubEvent::of_proposal(HubEventKind::ProposalAccepted, "p-1") });
    decided.messages.insert(2, accepted);
    assert_eq!(proposal_target(&decided, "r1", WakeCause::Message), None);

    let mut foreign = replied("sì");
    foreign.messages[1].event.as_mut().unwrap().workspace = Some("ws-2".into());
    assert_eq!(proposal_target(&foreign, "r1", WakeCause::Message), None);
}

// --- decide_proposal ---

#[test]
fn deciding_without_a_target_is_refused_and_touches_nothing() {
    let board = FakeBoard::with_pending(WS, "p-1", 2);
    let err = decide(&board, None, true).unwrap_err();
    assert!(err.starts_with("decide_proposal works only when the user's message"), "{err}");
    assert!(board.decisions.lock().unwrap().is_empty());
}

#[test]
fn accepting_queues_the_tasks_and_rejecting_queues_none() {
    let board = FakeBoard::with_pending(WS, "p-1", 2);
    let out = decide(&board, Some(&target("p-1")), true).unwrap();
    assert_eq!(out, json!({ "proposal": "p-1", "decision": "accepted", "tasks_queued": 2 }));
    assert_eq!(*board.decisions.lock().unwrap(), [(WS.to_string(), "p-1".to_string(), true)]);

    let board = FakeBoard::with_pending(WS, "p-2", 3);
    let out = decide(&board, Some(&target("p-2")), false).unwrap();
    assert_eq!(out, json!({ "proposal": "p-2", "decision": "rejected", "tasks_queued": 0 }));
    assert_eq!(*board.decisions.lock().unwrap(), [(WS.to_string(), "p-2".to_string(), false)]);
}

#[test]
fn a_proposal_no_longer_on_the_board_cannot_be_decided_twice() {
    let board = FakeBoard::with_pending(WS, "p-1", 2);
    decide(&board, Some(&target("p-1")), true).unwrap();
    let err = decide(&board, Some(&target("p-1")), false).unwrap_err();
    assert_eq!(err, NOT_PENDING);
    assert_eq!(board.decisions.lock().unwrap().len(), 1);
}

#[test]
fn a_board_refusal_goes_back_to_the_model() {
    let board =
        FakeBoard { refuse: Some("task id 'login' is already used".into()), ..FakeBoard::with_pending(WS, "p-1", 1) };
    let err = decide(&board, Some(&target("p-1")), true).unwrap_err();
    assert_eq!(err, "The proposal could not be accepted: task id 'login' is already used");
}

// --- open_plan_task ---

const GOAL: &str = "Add a login page with email and password\nKeep the current layout.";

fn channel_with(messages: Vec<Message>) -> ChannelThreads {
    let mut list = ChannelThreads::default();
    let mut t = thread(Vec::new());
    t.task_ids.clear();
    t.summary = Some("The user wants authentication.".into());
    list.threads.push(t);
    for m in messages {
        let new = NewMessage { id: m.id, author: m.author, body: m.body, reply_to: m.reply_to, event: m.event };
        list.add_message("t1", new, 1).unwrap();
    }
    list
}

fn ticket<'a>(channel: &'a Channel, trigger: &'a str) -> PlanTicket<'a> {
    PlanTicket { channel, thread_id: "t1", trigger_id: trigger, secretary: "giorgio", goal: GOAL }
}

#[test]
fn a_plan_task_is_created_linked_to_the_thread_with_the_secretary_as_owner() {
    let mut list = channel_with(vec![
        user("m1", "ciao"),
        bot("m2", "mario", "Ciao!"),
        user("m3", "@giorgio aggiungi il login\ncon email e password"),
    ]);
    let created = RefCell::new(Vec::new());
    let out = open_plan(&mut list, &ticket(&ws(), "m3"), 7, |workspace, order| {
        created.borrow_mut().push((workspace.to_string(), order.clone()));
        Ok("plan-login".to_string())
    })
    .unwrap();

    assert_eq!(out["task_id"], "plan-login");
    assert_eq!(out["title"], "Add a login page with email and password");
    let t = list.thread("t1").unwrap();
    assert_eq!((t.task_ids.as_slice(), t.owner.as_deref()), (&["plan-login".to_string()][..], Some("giorgio")));
    let created = created.into_inner();
    assert_eq!(created.len(), 1);
    let (workspace, order) = &created[0];
    assert_eq!(workspace, WS);
    let task = &order.task;
    assert!(
        task.contains("## User request (verbatim)\n\n> @giorgio aggiungi il login\n> con email e password"),
        "{task}"
    );
    assert!(task.contains(&format!("## Goal\n\n{GOAL}")), "{task}");
    assert!(task.contains("## Thread summary\n\nThe user wants authentication."), "{task}");
    assert!(task.contains("- [user] ciao\n- [@mario] Ciao!"), "{task}");
    assert!(task.contains("secretary @giorgio"), "{task}");
}

#[test]
fn in_the_general_channel_no_plan_is_created() {
    let mut list = channel_with(vec![user("m1", "@giorgio aggiungi il login")]);
    let err =
        open_plan(&mut list, &ticket(&Channel::General, "m1"), 1, |_, _| panic!("nessuna creazione")).unwrap_err();
    assert_eq!(err, GENERAL_CHANNEL);
    assert!(list.thread("t1").unwrap().owner.is_none());
}

#[test]
fn a_full_or_missing_thread_creates_nothing() {
    let mut list = channel_with(vec![user("m1", "x")]);
    let ids: Vec<String> = (0..TASKS_PER_THREAD_MAX).map(|i| format!("t-{i}")).collect();
    list.link_tasks("t1", &ids, 1).unwrap();
    let err = open_plan(&mut list, &ticket(&ws(), "m1"), 1, |_, _| panic!("nessuna creazione")).unwrap_err();
    assert_eq!(err, THREAD_FULL);
    let channel = ws();
    let gone = PlanTicket { thread_id: "nope", ..ticket(&channel, "m1") };
    assert_eq!(open_plan(&mut list, &gone, 1, |_, _| panic!("nessuna creazione")).unwrap_err(), THREAD_GONE);
}

#[test]
fn a_board_error_links_nothing() {
    let mut list = channel_with(vec![user("m1", "x")]);
    let err =
        open_plan(&mut list, &ticket(&ws(), "m1"), 1, |_, _| Err("the workspace is archived".into())).unwrap_err();
    assert_eq!(err, "The planning task could not be created: the workspace is archived");
    let t = list.thread("t1").unwrap();
    assert!(t.task_ids.is_empty() && t.owner.is_none());
}

#[test]
fn a_new_plan_after_the_questions_carries_the_answers_and_changes_owner() {
    let mut list = channel_with(vec![user("m1", "@mario aggiungi il login")]);
    open_plan(&mut list, &PlanTicket { secretary: "mario", ..ticket(&ws(), "m1") }, 1, |_, _| Ok("plan-a".into()))
        .unwrap();
    let mut questions = proposal_event("e1", "unused");
    questions.body = "Pianificazione conclusa senza proposta: Plan\nQuestions for the user:\n1. Quale database?".into();
    questions.event = Some(HubEvent::of_task(HubEventKind::TaskDone, "plan-a"));
    let added = vec![questions, user("m2", "@giorgio usa Postgres")];
    for m in added {
        let new = NewMessage { id: m.id, author: m.author, body: m.body, reply_to: None, event: m.event };
        list.add_message("t1", new, 2).unwrap();
    }
    let order = RefCell::new(None);
    open_plan(&mut list, &ticket(&ws(), "m2"), 3, |_, o| {
        *order.borrow_mut() = Some(o.clone());
        Ok("plan-b".into())
    })
    .unwrap();
    let t = list.thread("t1").unwrap();
    assert_eq!(
        (t.task_ids.as_slice(), t.owner.as_deref()),
        (&["plan-a".to_string(), "plan-b".to_string()][..], Some("giorgio"))
    );
    let task = order.into_inner().unwrap().task;
    assert!(task.contains("> @giorgio usa Postgres"), "{task}");
    assert!(task.contains("Questions for the user: / 1. Quale database?"), "{task}");
}

#[test]
fn the_plan_title_is_the_first_line_of_the_goal_clipped() {
    let t = thread(numbered(15));
    let long = format!("  \n{}\nsecond line", "a".repeat(200));
    let order = plan_order(&long, &t, "m14", "giorgio");
    assert_eq!(order.title.chars().count(), TITLE_KEPT + 1);
    assert!(order.title.chars().count() <= crate::board::TITLE_MAX);
    // Solo i 10 messaggi prima di quello del turno.
    assert!(!order.task.contains("- [user] messaggio 3\n") && order.task.contains("- [user] messaggio 4\n"));
    assert!(order.task.contains("> messaggio 14"));
    assert_eq!(plan_order("\u{7}", &t, "m14", "giorgio").title, FALLBACK_TITLE);
}

#[test]
fn an_event_wake_quotes_the_last_request_of_the_user() {
    let t = thread(vec![
        user("m1", "@giorgio aggiungi il login"),
        bot("m2", "giorgio", "Apro un piano."),
        proposal_event("e1", "p-1"),
    ]);
    let order = plan_order("Login", &t, "e1", "giorgio");
    assert!(order.task.contains("## User request (verbatim)\n\n> @giorgio aggiungi il login"), "{}", order.task);
    let none = plan_order("Login", &thread(vec![proposal_event("e1", "p-1")]), "e1", "giorgio");
    assert!(none.task.contains("(no message from the user)"));
}
