//! La verifica di una modifica senza applicarla (`boards::task_update_check`):
//! dice quello che direbbe la modifica vera, e la board non cambia.

use super::*;

fn paused_board() -> (AutomationState, FakeHost) {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    queue(&st, &h, vec![spec("a", &[]), spec("b", &["a"])]);
    (st, h)
}

#[test]
fn a_valid_change_passes_the_check_and_leaves_the_task_untouched() {
    let (st, h) = paused_board();
    let patch = json!({"title": "Nuovo", "task": "altro", "depends_on": [], "model": null});
    boards::task_update_check(&st, &h, "ws-1", "b", &patch).unwrap();
    let b = task(&st, &h, "b");
    assert_eq!((b["title"].as_str(), b["task"].as_str()), (Some("Task b"), Some("do b")));
    assert_eq!(b["depends_on"], json!(["a"]));
}

#[test]
fn the_check_refuses_what_the_update_refuses() {
    let (st, h) = paused_board();
    let cycle = json!({"depends_on": ["b"]});
    let e = boards::task_update_check(&st, &h, "ws-1", "a", &cycle).unwrap_err();
    assert!(e.contains("cycle"), "{e}");
    assert_eq!(boards::task_update(&st, &h, "ws-1", "a", &cycle).unwrap_err(), e);
    let e = boards::task_update_check(&st, &h, "ws-1", "a", &json!({"model": "not a model!"})).unwrap_err();
    assert!(e.contains("invalid model"), "{e}");
    assert!(boards::task_update_check(&st, &h, "ws-1", "zz", &json!({"title": "x"})).is_err());
    assert!(boards::task_update_check(&st, &h, "ws-9", "a", &json!({"title": "x"})).is_err());
}

#[test]
fn the_check_refuses_a_task_that_cannot_be_edited() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    let e = boards::task_update_check(&st, &h, "ws-1", "a", &json!({"title": "x"})).unwrap_err();
    assert!(e.contains("only queued or failed"), "{e}");
}
