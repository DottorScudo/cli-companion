use super::fixtures::{event_message, question_event, question_ref, task_event, thread, user};
use super::*;
use crate::hub::model::HubEventKind;

#[test]
fn a_question_waits_until_the_next_event_of_its_task() {
    let asked = thread(vec![question_event("e1", "a", "q-1-1")]);
    assert_eq!(pending_questions(&asked), [question_ref("a", "q-1-1")]);
    for next in [HubEventKind::TaskQuestionAnswered, HubEventKind::TaskFailed, HubEventKind::TaskDone] {
        let moved_on = thread(vec![question_event("e1", "a", "q-1-1"), task_event("e2", next, "a")]);
        assert!(pending_questions(&moved_on).is_empty(), "{next:?}");
    }
    let other_task = thread(vec![question_event("e1", "a", "q-1-1"), task_event("e2", HubEventKind::TaskDone, "b")]);
    assert_eq!(pending_questions(&other_task), [question_ref("a", "q-1-1")]);
}

#[test]
fn pending_questions_are_listed_oldest_first() {
    let t = thread(vec![
        question_event("e1", "b", "q-1-1"),
        user("m1", "ciao", None),
        question_event("e2", "a", "q-2-1"),
        question_event("e3", "c", "q-1-1"),
        task_event("e4", HubEventKind::TaskFailed, "c"),
    ]);
    assert_eq!(pending_questions(&t), [question_ref("b", "q-1-1"), question_ref("a", "q-2-1")]);
}

#[test]
fn a_question_needs_its_data_and_this_thread_workspace() {
    let mut no_data = question_event("e1", "a", "q-1-1");
    no_data.event.as_mut().unwrap().question = None;
    let mut elsewhere = question_event("e2", "b", "q-1-1");
    elsewhere.event.as_mut().unwrap().workspace = Some("ws-2".to_string());
    assert!(pending_questions(&thread(vec![no_data, elsewhere])).is_empty());
}

#[test]
fn a_reply_names_its_question_only_while_it_is_pending() {
    let t = thread(vec![question_event("e1", "a", "q-1-1"), user("m1", "ciao", None)]);
    assert_eq!(replied_question(&t, "e1"), Some(Ok(question_ref("a", "q-1-1"))));
    assert_eq!(replied_question(&t, "m1"), None, "un messaggio qualunque non e' una domanda");
    assert_eq!(replied_question(&t, "gone"), None);
    let proposal = event_message("p", crate::hub::model::HubEvent::of_proposal(HubEventKind::ProposalCreated, "p-1"));
    assert_eq!(replied_question(&thread(vec![proposal]), "p"), None);

    let answered = thread(vec![
        question_event("e1", "a", "q-1-1"),
        task_event("e2", HubEventKind::TaskQuestionAnswered, "a"),
        question_event("e3", "a", "q-1-2"),
    ]);
    assert_eq!(replied_question(&answered, "e1"), Some(Err(NotPending)), "una domanda gia' risposta");
    assert_eq!(replied_question(&answered, "e3"), Some(Ok(question_ref("a", "q-1-2"))));
    let concluded = thread(vec![question_event("e1", "a", "q-1-1"), task_event("e2", HubEventKind::TaskDone, "a")]);
    assert_eq!(replied_question(&concluded, "e1"), Some(Err(NotPending)), "il task e' concluso");
}
