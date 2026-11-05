//! Filtri, ricerca, ordine e pagine di `board_status` su una board di prova.

use super::*;
use crate::agents::Readiness;
use crate::board::{
    build_report, AgentGate, AgentUnavailableView, Board, BoardView, Finished, ReportFields, Task, TaskQuestion,
    TaskSpec, TaskState,
};
use TaskStatus::{Done, Failed, Queued, Running};

fn task(id: &str, title: &str, spec: &str, created_at: i64, status: TaskStatus) -> Task {
    let spec = TaskSpec { id: id.into(), title: title.into(), task: spec.into(), ..TaskSpec::default() };
    let mut t = Task::from_spec(spec, created_at);
    t.set_status(status);
    t
}

/// Cinque task, in ordine di inserimento `t-1..t-5`; dal piu' recente:
/// `t-4`, `t-3`, `t-2` (stessa ora di `t-3`, aggiunto prima), `t-1`, `t-5`.
/// Aspettano l'utente `t-3` (worker fermo), `t-4` (agente assente) e `t-5` (domanda).
fn view() -> BoardView {
    let mut t1 = task("t-1", "Login form", "Build the SIGN-IN page", 100, Done);
    let report = build_report("completed", "OAuth added", ReportFields::default(), None);
    t1.set_state(TaskState::Done(Finished { started_at: None, finished_at: 0, report: Some(report) }));
    let mut t2 = task("t-2", "Fix cache", "Evict stale keys", 200, Failed);
    t2.set_note(Some("boom in the parser"));
    let mut t3 = task("t-3", "Review login", "Check the sign-in page", 200, Running);
    t3.kind = TaskKind::Review;
    let t4 = task("t-4", "Docs", "Write the guide", 300, Queued);
    let t5 = task("t-5", "Old job", "Rotate the logs", 50, Running);
    let mut board = Board::new("ws-1");
    board.tasks = vec![t1, t2, t3, t4, t5];
    let mut view = board.view_with(None, false, &AgentGate::default());
    view.tasks[2].attention = true;
    view.tasks[3].agent_unavailable =
        Some(AgentUnavailableView { agent: "claude".into(), reason: Readiness::NotFound });
    view.tasks[4].question = Some(TaskQuestion {
        id: "q-1-1".into(),
        attempt: 1,
        text: "Which db?".into(),
        options: Vec::new(),
        asked_at: 0,
        answered_at: None,
        answer: None,
    });
    view
}

fn ids(page: &Page) -> Vec<String> {
    page.tasks.iter().map(|v| v.task.id.clone()).collect()
}

fn run(query: BoardQuery) -> Vec<String> {
    ids(&select(&view().tasks, &query))
}

fn search(text: &str) -> Vec<String> {
    run(BoardQuery { text: Some(text.into()), ..BoardQuery::default() })
}

#[test]
fn tasks_come_newest_first_and_ties_follow_the_reverse_insertion_order() {
    assert_eq!(run(BoardQuery::default()), ["t-4", "t-3", "t-2", "t-1", "t-5"]);
}

#[test]
fn each_filter_narrows_the_list() {
    let by_status = BoardQuery { statuses: vec![Done, Failed], ..BoardQuery::default() };
    assert_eq!(run(by_status), ["t-2", "t-1"]);
    let attention = BoardQuery { attention: true, ..BoardQuery::default() };
    assert_eq!(run(attention), ["t-4", "t-3", "t-5"]);
    let by_kind = BoardQuery { kind: Some(TaskKind::Review), ..BoardQuery::default() };
    assert_eq!(run(by_kind), ["t-3"]);
}

#[test]
fn filters_combine_in_and() {
    let running_for_the_user = BoardQuery { statuses: vec![Running], attention: true, ..BoardQuery::default() };
    assert_eq!(run(running_for_the_user), ["t-3", "t-5"]);
    let work_for_the_user =
        BoardQuery { attention: true, kind: Some(TaskKind::Work), text: Some("o".into()), ..BoardQuery::default() };
    assert_eq!(run(work_for_the_user), ["t-4", "t-5"], "t-3 e' una revisione");
    let nothing = BoardQuery { statuses: vec![Done], attention: true, ..BoardQuery::default() };
    assert!(run(nothing).is_empty());
}

#[test]
fn the_search_ignores_case_and_looks_in_every_text_of_the_task() {
    assert_eq!(search("sign-in"), ["t-3", "t-1"], "specifica");
    assert_eq!(search("LOGIN"), ["t-3", "t-1"], "titolo");
    assert_eq!(search("oauth"), ["t-1"], "riassunto del report");
    assert_eq!(search("BOOM"), ["t-2"], "nota");
    assert_eq!(search("t-4"), ["t-4"], "id");
    assert!(search("missing").is_empty());
}

#[test]
fn the_search_handles_unicode() {
    let mut v = view();
    v.tasks[0].task.title = "Città di Napoli".into();
    let query = BoardQuery { text: Some("CITTÀ".into()), ..BoardQuery::default() };
    assert_eq!(ids(&select(&v.tasks, &query)), ["t-1"]);
}

#[test]
fn pages_follow_the_filtered_order_and_the_last_has_no_next_offset() {
    let v = view();
    let page = |offset, limit| select(&v.tasks, &BoardQuery { offset, limit, ..BoardQuery::default() });
    let first = page(0, 2);
    assert_eq!((ids(&first), first.total, first.next_offset), (["t-4", "t-3"].map(String::from).to_vec(), 5, Some(2)));
    let second = page(2, 2);
    assert_eq!((ids(&second), second.next_offset), (["t-2", "t-1"].map(String::from).to_vec(), Some(4)));
    let last = page(4, 2);
    assert_eq!((ids(&last), last.total, last.next_offset), (vec!["t-5".to_string()], 5, None));
    let exact = page(3, 2);
    assert_eq!(exact.next_offset, None, "la pagina finisce esattamente sull'ultimo task");
}

#[test]
fn an_offset_past_the_end_is_an_empty_page_with_the_total() {
    let v = view();
    for offset in [5, 99, usize::MAX] {
        let page = select(&v.tasks, &BoardQuery { offset, limit: PAGE_MAX, ..BoardQuery::default() });
        assert_eq!((page.tasks.len(), page.total, page.next_offset), (0, 5, None), "offset {offset}");
    }
}

#[test]
fn the_total_counts_the_filtered_tasks_not_the_page() {
    let query = BoardQuery { attention: true, limit: 1, ..BoardQuery::default() };
    let v = view();
    let page = select(&v.tasks, &query);
    assert_eq!((ids(&page), page.total, page.next_offset), (vec!["t-4".to_string()], 3, Some(1)));
}
