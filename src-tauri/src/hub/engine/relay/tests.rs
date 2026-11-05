use super::*;
use crate::hub::model::HubEventKind;
use crate::hub::questions::fixtures::{question_event, question_ref, task_event, thread, user, FakeAnswers};

fn selector(question_id: Option<&str>, task_id: Option<&str>) -> QuestionSelector {
    QuestionSelector { question_id: question_id.map(str::to_string), task_id: task_id.map(str::to_string) }
}

/// Le domande `q-1-1` dei task `a` e `b` e `q-1-2` del task `c`, tutte in attesa.
fn three_pending() -> RelayCandidates {
    let t = thread(vec![
        question_event("e1", "a", "q-1-1"),
        question_event("e2", "b", "q-1-1"),
        question_event("e3", "c", "q-1-2"),
        user("m1", "@mario rispondi tu", None),
    ]);
    relay_candidates(&t, WakeCause::Message)
}

#[test]
fn the_candidates_are_the_pending_questions_in_a_user_turn() {
    let t = thread(vec![question_event("e1", "a", "q-1-1"), question_event("e2", "b", "q-1-1")]);
    assert_eq!(
        relay_candidates(&t, WakeCause::Message),
        Ok(vec![question_ref("a", "q-1-1"), question_ref("b", "q-1-1")])
    );
}

#[test]
fn a_board_event_turn_can_never_answer() {
    let t = thread(vec![question_event("e1", "a", "q-1-1")]);
    assert_eq!(relay_candidates(&t, WakeCause::BoardEvent), Err(NOT_FROM_USER));
}

#[test]
fn an_external_item_turn_can_never_answer() {
    let t = thread(vec![question_event("e1", "a", "q-1-1")]);
    assert_eq!(relay_candidates(&t, WakeCause::External), Err(NOT_FROM_USER));
    assert!(NOT_FROM_USER.contains("external content"));
}

#[test]
fn a_turn_that_cannot_answer_refuses_even_with_ids() {
    let t = thread(vec![question_event("e1", "a", "q-1-1")]);
    for cause in [WakeCause::BoardEvent, WakeCause::External] {
        let candidates = relay_candidates(&t, cause);
        assert_eq!(choose(&candidates, &selector(Some("q-1-1"), Some("a"))), Err(NOT_FROM_USER));
    }
}

#[test]
fn no_pending_question_leaves_nothing_to_choose() {
    let answered = thread(vec![
        question_event("e1", "a", "q-1-1"),
        task_event("e2", HubEventKind::TaskQuestionAnswered, "a"),
        user("m1", "@mario rispondi tu", None),
    ]);
    let candidates = relay_candidates(&answered, WakeCause::Message);
    assert_eq!(candidates, Err(NO_QUESTION));
    assert_eq!(choose(&candidates, &selector(Some("q-1-1"), None)), Err(NO_QUESTION));
}

#[test]
fn without_ids_the_only_pending_question_is_chosen() {
    let one = relay_candidates(&thread(vec![question_event("e1", "a", "q-1-1")]), WakeCause::Message);
    assert_eq!(choose(&one, &QuestionSelector::default()), Ok(question_ref("a", "q-1-1")));
}

#[test]
fn without_ids_several_pending_questions_cannot_be_told_apart() {
    assert_eq!(choose(&three_pending(), &QuestionSelector::default()), Err(SEVERAL_QUESTIONS));
    assert!(SEVERAL_QUESTIONS.contains("`question_id`"));
    assert!(SEVERAL_QUESTIONS.contains("Reply on that question message"));
}

#[test]
fn ids_choose_the_right_question_among_several() {
    let pending = three_pending();
    assert_eq!(choose(&pending, &selector(Some("q-1-1"), Some("b"))), Ok(question_ref("b", "q-1-1")));
    assert_eq!(choose(&pending, &selector(Some("q-1-2"), None)), Ok(question_ref("c", "q-1-2")));
    assert_eq!(choose(&pending, &selector(None, Some("a"))), Ok(question_ref("a", "q-1-1")));
}

#[test]
fn an_id_shared_by_several_tasks_needs_the_task() {
    assert_eq!(choose(&three_pending(), &selector(Some("q-1-1"), None)), Err(AMBIGUOUS_QUESTION));
}

#[test]
fn an_id_that_is_not_pending_in_the_thread_is_refused() {
    let pending = three_pending();
    for foreign in [
        selector(Some("q-9-9"), None),
        selector(Some("q-1-1"), Some("zzz")),
        selector(Some("q-1-2"), Some("a")),
        selector(None, Some("zzz")),
    ] {
        assert_eq!(choose(&pending, &foreign), Err(UNKNOWN_QUESTION), "{foreign:?}");
    }
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
