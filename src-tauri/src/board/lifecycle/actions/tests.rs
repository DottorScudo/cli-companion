//! Tabella delle azioni: chi puo' fare cosa in ogni stato, i testi degli errori
//! che ne derivano e il formato sul filo di `TaskAction`.

use super::*;
use crate::board::{Board, SpecContext, TaskSpec};
use serde_json::json;
use TaskAction::{Delete, DeleteCascade, MarkDone, Retry, Stop, ToTop};

const ALL_OPS: [Op; 8] = [
    Op::Action(Retry),
    Op::Action(MarkDone),
    Op::Action(Stop),
    Op::Action(Delete),
    Op::Action(DeleteCascade),
    Op::Action(ToTop),
    Op::Edit,
    Op::Reorder,
];

#[test]
fn the_table_says_what_each_status_permits() {
    let table: [(TaskStatus, &[Op]); 4] = [
        (
            TaskStatus::Queued,
            &[Op::Action(ToTop), Op::Action(Delete), Op::Action(DeleteCascade), Op::Edit, Op::Reorder],
        ),
        (TaskStatus::Running, &[Op::Action(Stop)]),
        (TaskStatus::Done, &[]),
        (
            TaskStatus::Failed,
            &[Op::Action(Retry), Op::Action(MarkDone), Op::Action(Delete), Op::Action(DeleteCascade), Op::Edit],
        ),
    ];
    for (status, permitted) in table {
        for op in ALL_OPS {
            assert_eq!(permits(status, op), permitted.contains(&op), "{status:?} {op:?}");
        }
    }
}

#[test]
fn the_action_list_has_every_action_once() {
    for (i, a) in TaskAction::ALL.iter().enumerate() {
        assert!(!TaskAction::ALL[..i].contains(a), "{a:?} twice");
    }
    for op in ALL_OPS {
        if let Op::Action(a) = op {
            assert!(TaskAction::ALL.contains(&a), "{a:?}");
        }
    }
}

#[test]
fn actions_use_snake_case_on_the_wire() {
    let wire = [
        (Retry, "retry"),
        (MarkDone, "mark_done"),
        (Stop, "stop"),
        (Delete, "delete"),
        (DeleteCascade, "delete_cascade"),
        (ToTop, "to_top"),
    ];
    assert_eq!(wire.map(|(a, _)| a), TaskAction::ALL);
    for (action, name) in wire {
        assert_eq!(serde_json::to_value(action).unwrap(), json!(name));
        assert_eq!(serde_json::from_value::<TaskAction>(json!(name)).unwrap(), action);
    }
    for bad in ["boh", "Retry", "markDone", ""] {
        assert!(serde_json::from_value::<TaskAction>(json!(bad)).is_err(), "{bad:?}");
    }
}

/// Una board con il task `a` nello stato `status` e un `q` in coda.
fn board_with(status: TaskStatus) -> Board {
    let mut b = Board::new("ws-1");
    for id in ["a", "q"] {
        let spec = TaskSpec { id: id.into(), title: id.into(), task: format!("do {id}"), ..TaskSpec::default() };
        b.create_task(spec, SpecContext::default(), 1).unwrap();
    }
    b.task_mut("a").unwrap().set_status(status);
    b
}

/// Prova `op` su `a` di una board nuova (le operazioni permesse la cambiano).
fn attempt(status: TaskStatus, op: Op) -> Result<(), String> {
    let mut b = board_with(status);
    match op {
        Op::Edit => b.update_task("a", &json!({"title": "T"}), SpecContext::default(), 2),
        Op::Action(Delete) => b.delete_task("a", 2).map(|_| ()),
        Op::Action(DeleteCascade) => b.delete_with_dependents("a", 2).map(|_| ()),
        Op::Action(Retry) => b.retry_task("a", 2),
        Op::Action(MarkDone) => b.mark_done("a", 2),
        Op::Reorder => b.move_task("a", None, 2),
        Op::Action(ToTop) => b.task_to_top("a", 2),
        Op::Action(Stop) => b.check_stop("a"),
    }
}

#[test]
fn every_guard_follows_the_table_and_reports_the_old_error_texts() {
    let verbs = [
        (Op::Edit, "queued or failed tasks can be updated"),
        (Op::Action(Delete), "queued or failed tasks can be deleted"),
        (Op::Action(DeleteCascade), "queued or failed tasks can be deleted"),
        (Op::Action(Retry), "failed tasks can be retried"),
        (Op::Action(MarkDone), "failed tasks can be marked done"),
        (Op::Reorder, "queued tasks can be reordered"),
        (Op::Action(ToTop), "queued tasks can be moved to the top"),
        (Op::Action(Stop), "running tasks can be stopped"),
    ];
    for status in TaskStatus::ALL {
        for (op, tail) in verbs {
            let expected =
                if permits(status, op) { Ok(()) } else { Err(format!("task 'a' is {}: only {tail}", status.as_str())) };
            assert_eq!(attempt(status, op), expected, "{status:?} {op:?}");
        }
    }
}

#[test]
fn an_unknown_task_is_not_found_whatever_the_operation() {
    let mut b = board_with(TaskStatus::Queued);
    let not_found = "task not found: zz";
    assert_eq!(b.update_task("zz", &json!({"title": "T"}), SpecContext::default(), 2), Err(not_found.into()));
    assert_eq!(b.delete_task("zz", 2).map(|_| ()), Err(not_found.into()));
    assert_eq!(b.delete_with_dependents("zz", 2).map(|_| ()), Err(not_found.into()));
    assert_eq!(b.retry_task("zz", 2), Err(not_found.into()));
    assert_eq!(b.mark_done("zz", 2), Err(not_found.into()));
    assert_eq!(b.move_task("zz", None, 2), Err(not_found.into()));
    assert_eq!(b.task_to_top("zz", 2), Err(not_found.into()));
    assert_eq!(b.check_stop("zz"), Err(not_found.into()));
}

#[test]
fn placing_before_a_task_that_is_not_queued_is_refused() {
    let mut b = board_with(TaskStatus::Done);
    let e = b.move_task("q", Some("a"), 2).unwrap_err();
    assert_eq!(e, "task 'a' is not queued: a task can only be placed before a queued task");
}
