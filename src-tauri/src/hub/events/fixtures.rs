//! Istantanee di prova per i test degli eventi della board.

use super::snapshot::{BoardSnapshot, ProposalSnap, ProposedTask, TaskSnap};
use crate::board::{TaskKind, TaskStatus};

pub const WS: &str = "ws-1";

/// Task `id` (titolo "Task <id>") nello stato `status`.
pub fn task(id: &str, status: TaskStatus) -> TaskSnap {
    TaskSnap {
        id: id.to_string(),
        title: format!("Task {id}"),
        kind: TaskKind::Work,
        status,
        attention: false,
        summary: None,
        verdict: None,
        note: None,
    }
}

/// Proposta `id` con i task `ids` (titoli "Task <id>").
pub fn proposal(id: &str, ids: &[&str]) -> ProposalSnap {
    ProposalSnap {
        id: id.to_string(),
        source_task: None,
        tasks: ids.iter().map(|t| ProposedTask { id: t.to_string(), title: format!("Task {t}") }).collect(),
    }
}

pub fn board(tasks: Vec<TaskSnap>, proposals: Vec<ProposalSnap>) -> BoardSnapshot {
    BoardSnapshot { workspace_id: WS.to_string(), rev: 0, tasks, proposals }
}
