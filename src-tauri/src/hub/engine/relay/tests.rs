use super::*;
use crate::hub::model::HubEventKind;
use crate::hub::questions::fixtures::{question_event, question_ref, task_event, thread, user, FakeAnswers};

#[test]
fn the_target_is_the_only_pending_question_in_a_user_turn() {
    let t = thread(vec![question_event("e1", "a", "q-1-1"), user("m1", "@mario digli di usare Postgres", None)]);
    assert_eq!(relay_target(&t, WakeCause::Message), Ok(question_ref("a", "q-1-1")));
}

#[test]
fn a_board_event_turn_can_never_answer() {
    let t = thread(vec![question_event("e1", "a", "q-1-1")]);
    assert_eq!(relay_target(&t, WakeCause::BoardEvent), Err(NOT_FROM_USER));
}

#[test]
fn an_external_item_turn_can_never_answer() {
    let t = thread(vec![question_event("e1", "a", "q-1-1")]);
    assert_eq!(relay_target(&t, WakeCause::External), Err(NOT_FROM_USER));
    assert!(NOT_FROM_USER.contains("external content"));
}

#[test]
fn no_question_or_several_questions_have_no_target() {
    let answered = thread(vec![
        question_event("e1", "a", "q-1-1"),
        task_event("e2", HubEventKind::TaskQuestionAnswered, "a"),
        user("m1", "@mario rispondi tu", None),
    ]);
    assert_eq!(relay_target(&answered, WakeCause::Message), Err(NO_QUESTION));
    let several = thread(vec![question_event("e1", "a", "q-1-1"), question_event("e2", "b", "q-1-1")]);
    assert_eq!(relay_target(&several, WakeCause::Message), Err(SEVERAL_QUESTIONS));
    assert!(SEVERAL_QUESTIONS.contains("Reply on that question message"));
}

#[test]
fn relay_delivers_the_body_to_the_target() {
    let board = FakeAnswers::default();
    let target = question_ref("a", "q-1-1");
    let value = relay(&board, Ok(&target), "Use the second option.").unwrap();
    assert_eq!(value, json!({ "task_id": "a", "question_id": "q-1-1", "delivered": true }));
    assert_eq!(board.delivered(), [(target, "Use the second option.".to_string())]);
}

#[test]
fn relay_without_target_or_with_a_board_error_reports_to_the_model() {
    let board = FakeAnswers::default();
    assert_eq!(relay(&board, Err(SEVERAL_QUESTIONS), "x"), Err(SEVERAL_QUESTIONS.to_string()));
    assert!(board.delivered().is_empty());
    let refusing = FakeAnswers::refusing("Il worker sta ancora lavorando.");
    let err = relay(&refusing, Ok(&question_ref("a", "q-1-1")), "x").unwrap_err();
    assert_eq!(err, "The answer could not be delivered to the worker: Il worker sta ancora lavorando.");
}
