//! L'istantanea compatta di una board: solo cio' che serve a riconoscere gli
//! eventi (stato, attenzione, titolo, tipo dei task; id e task delle proposte)
//! e a raccontarli (riassunto, verdetto, nota). Si ricava da `BoardView` e
//! resta in memoria per workspace finche' l'HUB e' acceso: i testi lunghi sono
//! gia' ripuliti e tagliati qui.

use crate::board::{BoardView, Proposal, TaskKind, TaskStatus, TaskView, Verdict};
use crate::hub::text::{clean_line, clean_text};

/// Caratteri tenuti di un titolo: un titolo della board ne ha al massimo 80,
/// il margine lascia posto al "+N" del titolo di un thread.
const TITLE_KEPT: usize = 100;
/// Caratteri tenuti di riassunto e nota: il messaggio resta breve, il resto e'
/// nella board.
const DETAIL_KEPT: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnap {
    pub id: String,
    pub title: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub attention: bool,
    /// Riassunto del report dell'ultimo tentativo concluso.
    pub summary: Option<String>,
    pub verdict: Option<Verdict>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedTask {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalSnap {
    pub id: String,
    /// Il task che l'ha proposta (il `plan` del pianificatore o la review del
    /// revisore); assente per le proposte dell'Intake.
    pub source_task: Option<String>,
    pub tasks: Vec<ProposedTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSnapshot {
    pub workspace_id: String,
    /// Revisione della board: riconosce una vista piu' vecchia della base.
    pub rev: u64,
    /// Nell'ordine della board.
    pub tasks: Vec<TaskSnap>,
    pub proposals: Vec<ProposalSnap>,
}

impl BoardSnapshot {
    pub fn of(view: &BoardView) -> Self {
        Self {
            workspace_id: view.workspace_id.clone(),
            rev: view.rev,
            tasks: view.tasks.iter().map(task_snap).collect(),
            proposals: view.proposals.iter().map(proposal_snap).collect(),
        }
    }

    pub fn task(&self, id: &str) -> Option<&TaskSnap> {
        self.tasks.iter().find(|t| t.id == id)
    }
}

fn task_snap(v: &TaskView) -> TaskSnap {
    let t = &v.task;
    let report = t.report();
    TaskSnap {
        id: t.id.clone(),
        title: clean_line(&t.title, TITLE_KEPT),
        kind: t.kind,
        status: t.status(),
        attention: v.attention,
        summary: report.map(|r| clean_text(&r.summary, DETAIL_KEPT)).filter(|s| !s.is_empty()),
        verdict: report.and_then(|r| r.verdict),
        note: t.note().map(|n| clean_text(n, DETAIL_KEPT)).filter(|s| !s.is_empty()),
    }
}

fn proposal_snap(p: &Proposal) -> ProposalSnap {
    ProposalSnap {
        id: p.id.clone(),
        source_task: p.source_task.clone(),
        tasks: p
            .tasks
            .iter()
            .map(|s| ProposedTask { id: s.id.clone(), title: clean_line(&s.title, TITLE_KEPT) })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{AgentGate, Board};

    #[test]
    fn the_snapshot_keeps_what_the_events_need() {
        let raw = serde_json::json!({
            "workspace_id": "ws-1",
            "tasks": [
                {"id": "r", "title": "Revisione", "task": "t", "kind": "review", "status": "done",
                 "report": {"status": "completed", "summary": "  tutto ok\u{7} ", "verdict": "passed"}},
                {"id": "f", "title": "Fallito", "task": "t", "status": "failed", "note": "x".repeat(DETAIL_KEPT + 5)},
                {"id": "q", "title": "In coda", "task": "t", "status": "queued"}
            ],
            "proposals": [{"id": "p-1", "source_tab": "intake", "created_at": 1,
                           "tasks": [{"id": "a", "title": "Nuovo", "task": "t"}]}]
        });
        let board = Board::from_json(&raw.to_string(), 1).unwrap();
        let snap = BoardSnapshot::of(&board.view_with(None, false, &AgentGate::default()));

        assert_eq!(snap.workspace_id, "ws-1");
        let review = snap.task("r").unwrap();
        assert_eq!(
            (review.kind, review.status, review.verdict),
            (TaskKind::Review, TaskStatus::Done, Some(Verdict::Passed))
        );
        assert_eq!(review.summary.as_deref(), Some("tutto ok"));
        let failed = snap.task("f").unwrap();
        assert_eq!(failed.note.as_ref().unwrap().chars().count(), DETAIL_KEPT + 1, "tagliata con …");
        let queued = snap.task("q").unwrap();
        assert_eq!((queued.status, queued.attention, &queued.summary), (TaskStatus::Queued, false, &None));
        assert_eq!(snap.tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["r", "f", "q"]);
        assert_eq!(
            snap.proposals,
            [ProposalSnap {
                id: "p-1".to_string(),
                source_task: None,
                tasks: vec![ProposedTask { id: "a".to_string(), title: "Nuovo".to_string() }]
            }]
        );
    }
}
