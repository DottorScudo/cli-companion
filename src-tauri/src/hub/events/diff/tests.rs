use super::*;
use crate::hub::events::fixtures::{board, proposal, task};

fn changes(events: &[BoardEvent]) -> Vec<(TaskChange, &str)> {
    events
        .iter()
        .filter_map(|e| match e {
            BoardEvent::Task { change, task } => Some((*change, task.id.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn identical_boards_have_no_events() {
    let b = board(vec![task("a", TaskStatus::Running)], vec![proposal("p-1", &["x"])]);
    assert!(diff(&b, &b).is_empty());
}

#[test]
fn a_new_proposal_is_an_event_with_its_tasks() {
    let prev = board(vec![], vec![]);
    let next = board(vec![], vec![proposal("p-1", &["a", "b"])]);
    assert_eq!(diff(&prev, &next), [BoardEvent::ProposalCreated(proposal("p-1", &["a", "b"]))]);
}

#[test]
fn an_accepted_proposal_brings_its_tasks_without_creation_events() {
    let prev = board(vec![], vec![proposal("p-1", &["a", "b"])]);
    let next = board(vec![task("a", TaskStatus::Queued), task("b", TaskStatus::Queued)], vec![]);
    let accepted = BoardEvent::ProposalAccepted {
        proposal_id: "p-1".to_string(),
        task_ids: vec!["a".to_string(), "b".to_string()],
    };
    assert_eq!(diff(&prev, &next), [accepted]);
}

#[test]
fn a_task_of_an_accepted_proposal_that_already_started_reports_the_start() {
    let prev = board(vec![], vec![proposal("p-1", &["a"])]);
    let next = board(vec![task("a", TaskStatus::Running)], vec![]);
    let events = diff(&prev, &next);
    assert!(matches!(events[0], BoardEvent::ProposalAccepted { .. }));
    assert_eq!(changes(&events), [(TaskChange::Started, "a")]);
}

#[test]
fn a_discarded_proposal_brings_no_tasks() {
    let prev = board(vec![task("x", TaskStatus::Done)], vec![proposal("p-1", &["a"])]);
    let next = board(vec![task("x", TaskStatus::Done)], vec![]);
    assert_eq!(diff(&prev, &next), [BoardEvent::ProposalRejected { proposal_id: "p-1".to_string() }]);
}

#[test]
fn a_task_that_does_not_come_from_a_proposal_is_created_by_hand() {
    let prev = board(vec![], vec![proposal("p-1", &["b"])]);
    let next = board(vec![task("a", TaskStatus::Queued)], vec![proposal("p-1", &["b"])]);
    assert_eq!(changes(&diff(&prev, &next)), [(TaskChange::Created, "a")]);
}

#[test]
fn status_changes_are_events_but_requeueing_is_not() {
    let cases = [
        (TaskStatus::Queued, TaskStatus::Running, Some(TaskChange::Started)),
        (TaskStatus::Running, TaskStatus::Done, Some(TaskChange::Done)),
        (TaskStatus::Running, TaskStatus::Failed, Some(TaskChange::Failed)),
        (TaskStatus::Failed, TaskStatus::Done, Some(TaskChange::Done)),
        (TaskStatus::Failed, TaskStatus::Running, Some(TaskChange::Started)),
        (TaskStatus::Failed, TaskStatus::Queued, None),
        (TaskStatus::Running, TaskStatus::Queued, None),
    ];
    for (before, after, expected) in cases {
        let events = diff(&board(vec![task("a", before)], vec![]), &board(vec![task("a", after)], vec![]));
        let expected: Vec<(TaskChange, &str)> = expected.map(|c| (c, "a")).into_iter().collect();
        assert_eq!(changes(&events), expected, "{before:?} -> {after:?}");
    }
}

#[test]
fn the_completed_task_carries_its_current_data() {
    let mut done = task("r", TaskStatus::Done);
    (done.summary, done.verdict) = (Some("tutto ok".to_string()), Some(crate::board::Verdict::Passed));
    let events = diff(&board(vec![task("r", TaskStatus::Running)], vec![]), &board(vec![done.clone()], vec![]));
    assert_eq!(events, [BoardEvent::Task { change: TaskChange::Done, task: done }]);
}

#[test]
fn attention_is_reported_when_it_starts_only() {
    let mut waiting = task("a", TaskStatus::Running);
    waiting.attention = true;
    let running = board(vec![task("a", TaskStatus::Running)], vec![]);
    let attention = board(vec![waiting], vec![]);
    assert_eq!(changes(&diff(&running, &attention)), [(TaskChange::Attention, "a")]);
    assert!(diff(&attention, &attention).is_empty());
    assert!(diff(&attention, &running).is_empty());
}

#[test]
fn a_task_gone_without_being_done_is_deleted_but_an_archived_done_task_is_not() {
    let prev =
        board(vec![task("q", TaskStatus::Queued), task("f", TaskStatus::Failed), task("d", TaskStatus::Done)], vec![]);
    let events = diff(&prev, &board(vec![], vec![]));
    assert_eq!(changes(&events), [(TaskChange::Deleted, "q"), (TaskChange::Deleted, "f")]);
    // Il task eliminato porta i dati che aveva.
    assert!(matches!(&events[0], BoardEvent::Task { task, .. } if task.title == "Task q"));
}

#[test]
fn proposals_come_before_tasks_and_deletions_last() {
    let prev = board(vec![task("old", TaskStatus::Queued)], vec![]);
    let next = board(vec![task("a", TaskStatus::Queued)], vec![proposal("p-1", &["b"])]);
    let events = diff(&prev, &next);
    assert!(matches!(events[0], BoardEvent::ProposalCreated(_)));
    assert_eq!(changes(&events), [(TaskChange::Created, "a"), (TaskChange::Deleted, "old")]);
}
