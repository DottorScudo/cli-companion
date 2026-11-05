use super::*;
use crate::hub::model::HubEventKind;
use crate::hub::questions::fixtures::{question_event, question_ref, task_event, thread, user, FakeAnswers};

#[test]
fn a_reply_to_a_pending_question_reaches_the_worker_as_written() {
    let body = "  Usa Postgres, non SQLite.\nGrazie  ";
    let reply = user("m1", body, Some("e1"));
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::default();
    assert_eq!(answer_reply(&t, &reply, &board).as_deref(), Some(ANSWER_SENT));
    assert_eq!(board.delivered(), [(question_ref("a", "q-1-1"), body.to_string())]);
}

#[test]
fn a_delivery_error_is_reported_with_the_terminal_hint() {
    let reply = user("m1", "sì", Some("e1"));
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::refusing("Il worker sta ancora lavorando.");
    let outcome = answer_reply(&t, &reply, &board).unwrap();
    assert_eq!(outcome, format!("Risposta non inviata: Il worker sta ancora lavorando. {TERMINAL_HINT}"));
}

#[test]
fn a_question_that_no_longer_waits_is_a_clear_error_without_delivery() {
    let board = FakeAnswers::default();
    for next in [HubEventKind::TaskQuestionAnswered, HubEventKind::TaskDone, HubEventKind::TaskFailed] {
        let reply = user("m1", "sì", Some("e1"));
        let t = thread(vec![question_event("e1", "a", "q-1-1"), task_event("e2", next, "a"), reply.clone()]);
        assert_eq!(answer_reply(&t, &reply, &board).as_deref(), Some(NOT_PENDING), "{next:?}");
    }
    assert!(board.delivered().is_empty());
}

#[test]
fn other_messages_follow_the_normal_path() {
    let board = FakeAnswers::default();
    let t = thread(vec![question_event("e1", "a", "q-1-1"), user("m0", "ciao", None)]);
    assert_eq!(answer_reply(&t, &user("m1", "@mario stato?", None), &board), None);
    assert_eq!(answer_reply(&t, &user("m2", "ok", Some("m0")), &board), None);
    let mut by_bot = user("m3", "sì", Some("e1"));
    by_bot.author = Author::Bot { name: "mario".into() };
    assert_eq!(answer_reply(&t, &by_bot, &board), None, "solo l'utente risponde direttamente");
    assert!(board.delivered().is_empty());
}
