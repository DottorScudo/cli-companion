//! `task_action` e `board_control` su una board vera, scritta da un finto
//! `BoardWriter` che applica le stesse operazioni della board.

use super::*;
use crate::board::{AgentGate, Board, Task, TaskSpec, TaskStatus};
use std::sync::Mutex;
use TaskStatus::{Done, Failed, Queued, Running};

const NAME: &str = "Muse Companion";

fn task(id: &str, status: TaskStatus) -> Task {
    let spec = TaskSpec { id: id.into(), title: format!("Title {id}"), task: "spec".into(), ..TaskSpec::default() };
    let mut t = Task::from_spec(spec, 0);
    t.set_status(status);
    t
}

/// La board `ws-1` scritta dai test, e le scritture ricevute.
struct FakeWriter {
    board: Mutex<Board>,
    writes: Mutex<Vec<String>>,
    refuse: Option<String>,
}

impl FakeWriter {
    /// `f-1` fallito, `r-1` in corso, `q-1`..`q-3` in coda, `d-1` fatto.
    fn new() -> Self {
        let mut board = Board::new("ws-1");
        board.tasks = vec![
            task("f-1", Failed),
            task("r-1", Running),
            task("q-1", Queued),
            task("q-2", Queued),
            task("q-3", Queued),
            task("d-1", Done),
        ];
        Self { board: Mutex::new(board), writes: Mutex::default(), refuse: None }
    }

    fn view(&self) -> BoardView {
        self.board.lock().unwrap().view_with(None, false, &AgentGate::default())
    }

    fn write(&self, what: String, f: impl FnOnce(&mut Board) -> Result<(), String>) -> Result<BoardView, String> {
        if let Some(e) = &self.refuse {
            return Err(e.clone());
        }
        f(&mut self.board.lock().unwrap())?;
        self.writes.lock().unwrap().push(what);
        Ok(self.view())
    }

    fn writes(&self) -> Vec<String> {
        self.writes.lock().unwrap().clone()
    }

    fn order(&self) -> Vec<String> {
        self.board.lock().unwrap().tasks.iter().map(|t| t.id.clone()).collect()
    }
}

impl BoardWriter for FakeWriter {
    fn act_on_task(&self, workspace: &str, task_id: &str, action: TaskAction) -> Result<BoardView, String> {
        self.write(format!("{workspace} {task_id} {action:?}"), |b| match action {
            TaskAction::Retry => b.retry_task(task_id, 1),
            TaskAction::ToTop => b.task_to_top(task_id, 1),
            other => panic!("azione non attesa: {other:?}"),
        })
    }

    fn move_task(&self, workspace: &str, task_id: &str, before: &str) -> Result<BoardView, String> {
        self.write(format!("{workspace} {task_id} before {before}"), |b| b.move_task(task_id, Some(before), 1))
    }

    fn set_board(&self, workspace: &str, change: BoardChange) -> Result<BoardView, String> {
        self.write(format!("{workspace} {change:?}"), |b| {
            if let Some(paused) = change.paused {
                b.paused = paused;
            }
            if let Some(max) = change.max_parallel {
                b.max_parallel = max;
            }
            Ok(())
        })
    }
}

fn change(writer: &FakeWriter, task_id: &str, op: TaskOp) -> Result<Value, String> {
    change_task(writer, &writer.view(), NAME, task_id, &op)
}

fn before(id: &str) -> TaskOp {
    TaskOp::MoveBefore { before_task_id: id.into() }
}

#[test]
fn only_a_turn_woken_by_the_user_can_change_the_board() {
    assert_eq!(require_user_request(WakeCause::Message), Ok(()));
    for cause in [WakeCause::BoardEvent, WakeCause::External] {
        assert!(require_user_request(cause).unwrap_err().contains("never when a board event or external content"));
    }
}

#[test]
fn a_failed_task_is_retried() {
    let w = FakeWriter::new();
    let out = change(&w, "f-1", TaskOp::Retry).unwrap();
    assert_eq!(out, json!({ "workspace": NAME, "task_id": "f-1", "title": "Title f-1", "status": "queued" }));
    assert_eq!(w.writes(), ["ws-1 f-1 Retry"]);
}

#[test]
fn a_queued_task_goes_to_the_top_or_before_another() {
    let w = FakeWriter::new();
    assert_eq!(change(&w, "q-3", TaskOp::ToTop).unwrap()["status"], "queued");
    assert_eq!(w.order(), ["f-1", "r-1", "q-3", "q-1", "q-2", "d-1"]);
    let out = change(&w, "q-1", before("q-3")).unwrap();
    assert_eq!((out["before_task_id"].as_str(), out["before_title"].as_str()), (Some("q-3"), Some("Title q-3")));
    assert_eq!(w.order(), ["f-1", "r-1", "q-1", "q-3", "q-2", "d-1"]);
    assert_eq!(w.writes(), ["ws-1 q-3 ToTop", "ws-1 q-1 before q-3"]);
}

#[test]
fn an_action_the_task_does_not_allow_is_refused_without_writing() {
    let w = FakeWriter::new();
    let retry = change(&w, "q-1", TaskOp::Retry).unwrap_err();
    assert_eq!(
        retry,
        "Task \"q-1\" (\"Title q-1\") is queued and cannot be retried now. Only a failed task can be retried."
    );
    assert!(change(&w, "d-1", TaskOp::Retry).unwrap_err().contains("is done and cannot be retried"));
    assert!(change(&w, "f-1", TaskOp::ToTop).unwrap_err().contains("is failed and cannot be moved to the top"));
    assert!(change(&w, "r-1", before("q-1")).unwrap_err().contains("is running and cannot be reordered"));
    assert!(change(&w, "q-1", before("d-1")).unwrap_err().contains("\"d-1\" (\"Title d-1\") is done"));
    assert!(change(&w, "q-1", before("q-1")).unwrap_err().contains("different task"));
    assert!(w.writes().is_empty());
}

#[test]
fn a_task_that_is_not_on_this_board_is_refused() {
    let w = FakeWriter::new();
    for op in [TaskOp::Retry, TaskOp::ToTop, before("q-1")] {
        let e = change(&w, "other-ws-task", op).unwrap_err();
        assert!(e.starts_with("No task with id \"other-ws-task\" on the board of workspace \"Muse Companion\""), "{e}");
    }
    assert!(change(&w, "q-1", before("nope\nx")).unwrap_err().contains("\"nope x\""));
    assert!(w.writes().is_empty());
}

#[test]
fn a_refusal_of_the_board_reaches_the_model() {
    let w = FakeWriter { refuse: Some("workspace archiviato".into()), ..FakeWriter::new() };
    assert_eq!(
        change(&w, "f-1", TaskOp::Retry).unwrap_err(),
        "The board did not accept the change: workspace archiviato"
    );
    let e = control_board(&w, &w.view(), NAME, BoardChange { paused: Some(true), max_parallel: None }).unwrap_err();
    assert!(e.ends_with("workspace archiviato"));
}

#[test]
fn the_board_is_paused_resumed_and_its_parallelism_set() {
    let w = FakeWriter::new();
    let pause = BoardChange { paused: Some(true), max_parallel: Some(5) };
    let out = control_board(&w, &w.view(), NAME, pause).unwrap();
    assert_eq!(
        out,
        json!({ "workspace": NAME, "paused": true, "max_parallel": 5, "changed": { "paused": true, "max_parallel": 5 } })
    );
    let resume = BoardChange { paused: Some(false), max_parallel: None };
    assert_eq!(control_board(&w, &w.view(), NAME, resume).unwrap()["changed"], json!({ "paused": false }));
    assert_eq!(w.writes().len(), 2);
}

#[test]
fn only_what_changes_is_written() {
    let w = FakeWriter::new();
    let current = w.view().max_parallel;
    let same = BoardChange { paused: Some(false), max_parallel: Some(current) };
    let out = control_board(&w, &w.view(), NAME, same).unwrap();
    assert_eq!(out["changed"], json!({}));
    assert!(out["note"].as_str().unwrap().contains("nothing was changed"));
    assert!(w.writes().is_empty());
    let partly = BoardChange { paused: Some(true), max_parallel: Some(current) };
    assert_eq!(control_board(&w, &w.view(), NAME, partly).unwrap()["changed"], json!({ "paused": true }));
    assert_eq!(w.writes(), [format!("ws-1 {:?}", BoardChange { paused: Some(true), max_parallel: None })]);
}
