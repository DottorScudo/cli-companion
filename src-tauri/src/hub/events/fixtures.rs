//! Istantanee di prova per i test degli eventi della board.

use super::snapshot::{BoardSnapshot, ProposalSnap, ProposedTask, TaskSnap};
use crate::board::{TaskKind, TaskStatus};
use crate::hub::model::HubQuestion;

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
        summary_chars: None,
        plan_report: None,
        verdict: None,
        note: None,
        note_chars: None,
        question: None,
    }
}

/// Task `id` in corso con la domanda `question_id` in attesa.
pub fn asking(id: &str, question_id: &str, options: &[&str]) -> TaskSnap {
    let question = HubQuestion {
        id: question_id.to_string(),
        text: "Quale database uso?".to_string(),
        options: options.iter().map(|o| o.to_string()).collect(),
    };
    TaskSnap { question: Some(question), ..task(id, TaskStatus::Running) }
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
