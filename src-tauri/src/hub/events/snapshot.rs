//! L'istantanea compatta di una board: solo cio' che serve a riconoscere gli
//! eventi (stato, attenzione, domanda in attesa, titolo, tipo dei task; id e
//! task delle proposte) e a raccontarli (riassunto, verdetto, nota, testo e
//! opzioni della domanda). Si ricava da `BoardView` e
//! resta in memoria per workspace finche' l'HUB e' acceso: i testi lunghi sono
//! gia' ripuliti e accorciati qui, con la lunghezza del testo intero accanto
//! (che resta nella board); solo il riassunto di un task `plan` si tiene anche
//! intero, per riportare tutte le domande del pianificatore.

use crate::board::{
    BoardView, Proposal, TaskKind, TaskQuestion, TaskStatus, TaskView, Verdict, OPTION_MAX, QUESTION_MAX,
};
use crate::hub::model::HubQuestion;
use crate::hub::text::{clean_line, clean_text};
use crate::validate::clip_with_ellipsis;

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
    /// Riassunto del report dell'ultimo tentativo concluso, accorciato.
    pub summary: Option<String>,
    /// Lunghezza del riassunto intero, solo se `summary` e' accorciato.
    pub summary_chars: Option<usize>,
    /// Il riassunto intero di un task `plan`: le domande del pianificatore,
    /// riportate per intero se il piano finisce senza proposta.
    pub plan_report: Option<String>,
    pub verdict: Option<Verdict>,
    /// La nota del task, accorciata.
    pub note: Option<String>,
    /// Lunghezza della nota intera, solo se `note` e' accorciata.
    pub note_chars: Option<usize>,
    /// La domanda del worker in attesa di risposta.
    pub question: Option<HubQuestion>,
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
    let summary = report.and_then(|r| Detail::of(&r.summary));
    let note = t.note().and_then(Detail::of);
    let plan_report = summary.as_ref().filter(|_| t.kind == TaskKind::Plan).map(|s| s.full.clone());
    let (summary, summary_chars) = Detail::shortened(summary);
    let (note, note_chars) = Detail::shortened(note);
    TaskSnap {
        id: t.id.clone(),
        title: clean_line(&t.title, TITLE_KEPT),
        kind: t.kind,
        status: t.status(),
        attention: v.attention,
        summary,
        summary_chars,
        plan_report,
        verdict: report.and_then(|r| r.verdict),
        note,
        note_chars,
        question: v.question.as_ref().map(question_snap),
    }
}

/// Un testo lungo del task (riassunto, nota) ripulito: intero e con la sua lunghezza.
struct Detail {
    full: String,
    chars: usize,
}

impl Detail {
    /// `None` se non resta testo dopo la pulizia.
    fn of(raw: &str) -> Option<Self> {
        let full = clean_text(raw, usize::MAX);
        let chars = full.chars().count();
        (chars > 0).then_some(Self { full, chars })
    }

    /// Il testo accorciato a `DETAIL_KEPT` caratteri (con "…") e, solo se
    /// accorciato, la lunghezza del testo intero.
    fn shortened(detail: Option<Self>) -> (Option<String>, Option<usize>) {
        match detail {
            None => (None, None),
            Some(d) if d.chars <= DETAIL_KEPT => (Some(d.full), None),
            Some(d) => (Some(clip_with_ellipsis(&d.full, DETAIL_KEPT)), Some(d.chars)),
        }
    }
}

/// La domanda ripulita dai caratteri di controllo (la board ne ha gia' fissato i limiti).
fn question_snap(q: &TaskQuestion) -> HubQuestion {
    HubQuestion {
        id: q.id.clone(),
        text: clean_text(&q.text, QUESTION_MAX),
        options: q.options.iter().map(|o| clean_line(o, OPTION_MAX)).filter(|o| !o.is_empty()).collect(),
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
    fn a_pending_question_is_kept_cleaned() {
        let q = TaskQuestion {
            id: "q-1-1".into(),
            attempt: 1,
            text: " Quale\u{1b}[31m database?\nPostgres o SQLite ".into(),
            options: vec!["Postgres\u{7}".into(), "\u{7}".into()],
            asked_at: 1,
            answered_at: None,
            answer: None,
        };
        let snap = question_snap(&q);
        assert_eq!(snap.id, "q-1-1");
        assert!(!snap.text.contains('\u{1b}') && snap.text.contains('\n'), "{:?}", snap.text);
        assert_eq!(snap.options, ["Postgres"], "un'opzione vuota dopo la pulizia si scarta");
    }

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
        assert_eq!((review.summary_chars, &review.plan_report), (None, &None));
        let failed = snap.task("f").unwrap();
        assert_eq!(failed.note.as_ref().unwrap().chars().count(), DETAIL_KEPT + 1, "tagliata con …");
        assert_eq!(failed.note_chars, Some(DETAIL_KEPT + 5));
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

    #[test]
    fn a_plan_keeps_its_whole_report_and_a_short_text_has_no_length() {
        let questions = format!("Questions for the user:\n{}", "1. Quale database?\n".repeat(1000));
        let raw = serde_json::json!({
            "workspace_id": "ws-1",
            "tasks": [
                {"id": "p", "title": "Piano", "task": "t", "kind": "plan", "status": "done",
                 "report": {"status": "completed", "summary": questions}},
                {"id": "f", "title": "Fallito", "task": "t", "status": "failed", "note": "è".repeat(DETAIL_KEPT)}
            ]
        });
        let board = Board::from_json(&raw.to_string(), 1).unwrap();
        let snap = BoardSnapshot::of(&board.view_with(None, false, &AgentGate::default()));

        let plan = snap.task("p").unwrap();
        let whole = questions.trim_end();
        assert_eq!(plan.plan_report.as_deref(), Some(whole));
        assert_eq!(plan.summary_chars, Some(whole.chars().count()));
        assert!(plan.summary.as_ref().unwrap().ends_with('…'));
        let failed = snap.task("f").unwrap();
        assert_eq!((failed.note.as_ref().unwrap().chars().count(), failed.note_chars), (DETAIL_KEPT, None));
    }
}
