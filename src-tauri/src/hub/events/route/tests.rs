use super::*;
use crate::board::{TaskKind, Verdict};
use crate::hub::events::diff::diff;
use crate::hub::events::fixtures::{board, proposal, task, WS};
use crate::hub::model::{MessageKind, Thread};

fn channel() -> Channel {
    Channel::Workspace(WS.to_string())
}

/// Un passo della board da `prev` a `next` all'istante `now` (id unici per istante).
fn step(list: &mut ChannelThreads, prev: &BoardSnapshot, next: &BoardSnapshot, now: i64) -> Vec<ThreadSummary> {
    let mut n = 0;
    route(list, &channel(), next, &diff(prev, next), now, &mut |prefix| {
        n += 1;
        format!("{prefix}-{now}-{n}")
    })
}

fn only_thread(list: &ChannelThreads) -> &Thread {
    assert_eq!(list.threads.len(), 1, "{:?}", list.threads);
    &list.threads[0]
}

fn last_event(thread: &Thread) -> &HubEvent {
    thread.messages.iter().rev().find_map(|m| m.event.as_ref()).expect("un evento")
}

fn bodies(thread: &Thread) -> Vec<&str> {
    thread.messages.iter().map(|m| m.body.as_str()).collect()
}

#[test]
fn a_proposal_opens_a_thread_named_after_its_first_task() {
    let mut list = ChannelThreads::default();
    let touched = step(&mut list, &board(vec![], vec![]), &board(vec![], vec![proposal("p-1", &["a", "b"])]), 10);
    let t = only_thread(&list);
    assert_eq!((t.title.as_str(), t.proposal_id.as_deref(), t.state), ("Task a +1", Some("p-1"), ThreadState::Open));
    assert_eq!(bodies(t), ["Nuova proposta: 2 task\n- Task a\n- Task b"]);
    let m = &t.messages[0];
    assert_eq!((m.kind, &m.author), (MessageKind::Event, &Author::System));
    let event = last_event(t);
    assert_eq!(
        (event.kind, event.proposal_id.as_deref(), event.workspace.as_deref()),
        (HubEventKind::ProposalCreated, Some("p-1"), Some(WS))
    );
    assert_eq!(touched.len(), 1);
    assert!(touched[0].waiting_user);
}

#[test]
fn accepting_a_proposal_links_its_tasks_in_the_same_thread() {
    let mut list = ChannelThreads::default();
    let pending = board(vec![], vec![proposal("p-1", &["a", "b"])]);
    step(&mut list, &board(vec![], vec![]), &pending, 10);
    let accepted = board(vec![task("a", TaskStatus::Queued), task("b", TaskStatus::Queued)], vec![]);
    let touched = step(&mut list, &pending, &accepted, 20);
    let t = only_thread(&list);
    assert_eq!(t.task_ids, ["a", "b"]);
    assert_eq!(bodies(t)[1], "Proposta accettata: 2 task in coda.");
    assert_eq!(last_event(t).kind, HubEventKind::ProposalAccepted);
    assert!(!touched[0].waiting_user, "una proposta decisa non aspetta piu'");
}

#[test]
fn accepting_a_proposal_seen_before_the_hub_was_on_creates_its_thread() {
    let mut list = ChannelThreads::default();
    let pending = board(vec![], vec![proposal("p-1", &["a"])]);
    step(&mut list, &pending, &board(vec![task("a", TaskStatus::Queued)], vec![]), 20);
    let t = only_thread(&list);
    assert_eq!(
        (t.title.as_str(), t.proposal_id.as_deref(), t.task_ids.as_slice()),
        ("Task a", Some("p-1"), &["a".to_string()][..])
    );
}

#[test]
fn discarding_a_proposal_closes_its_thread() {
    let mut list = ChannelThreads::default();
    let pending = board(vec![], vec![proposal("p-1", &["a"])]);
    step(&mut list, &board(vec![], vec![]), &pending, 10);
    step(&mut list, &pending, &board(vec![], vec![]), 20);
    let t = only_thread(&list);
    assert_eq!(t.state, ThreadState::Closed);
    assert_eq!(bodies(t)[1], PROPOSAL_REJECTED);
    assert_eq!(last_event(t).kind, HubEventKind::ProposalRejected);
}

#[test]
fn discarding_a_proposal_without_a_thread_writes_nothing() {
    let mut list = ChannelThreads::default();
    let touched = step(&mut list, &board(vec![], vec![proposal("p-1", &["a"])]), &board(vec![], vec![]), 20);
    assert!(touched.is_empty() && list.threads.is_empty());
}

#[test]
fn a_task_created_by_hand_gets_its_own_thread() {
    let mut list = ChannelThreads::default();
    step(&mut list, &board(vec![], vec![]), &board(vec![task("a", TaskStatus::Queued)], vec![]), 10);
    let t = only_thread(&list);
    assert_eq!((t.title.as_str(), t.task_ids.as_slice()), ("Task a", &["a".to_string()][..]));
    assert_eq!(bodies(t), ["Task creato: Task a"]);
    assert_eq!(last_event(t).kind, HubEventKind::TaskCreated);
}

#[test]
fn task_events_go_to_the_thread_of_their_task() {
    let mut list = ChannelThreads::default();
    let queued = board(vec![task("a", TaskStatus::Queued), task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &board(vec![], vec![proposal("p-1", &["a", "b"])]), &queued, 10);
    let running = board(vec![task("a", TaskStatus::Running), task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &queued, &running, 20);
    let mut waiting = task("a", TaskStatus::Running);
    waiting.attention = true;
    let attention = board(vec![waiting, task("b", TaskStatus::Queued)], vec![]);
    let touched = step(&mut list, &running, &attention, 30);
    assert!(touched[0].waiting_user);
    let mut failed = task("a", TaskStatus::Failed);
    failed.note = Some("worker interrotto".to_string());
    let failed_board = board(vec![failed, task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &attention, &failed_board, 40);

    let t = only_thread(&list);
    assert_eq!(
        bodies(t)[1..],
        ["Task avviato: Task a", "Il task richiede attenzione: Task a", "Task fallito: Task a\nworker interrotto"]
    );
    let event = last_event(t);
    assert_eq!(event.kind, HubEventKind::TaskFailed);
    assert_eq!((event.task_id.as_deref(), event.status.as_deref()), (Some("a"), Some("failed")));
    assert_eq!(event.note.as_deref(), Some("worker interrotto"));
}

#[test]
fn a_completed_review_reports_its_summary_and_verdict() {
    let mut list = ChannelThreads::default();
    let mut review = task("r", TaskStatus::Running);
    review.kind = TaskKind::Review;
    let running = board(vec![review.clone()], vec![]);
    step(&mut list, &board(vec![], vec![]), &running, 10);
    (review.status, review.summary, review.verdict) =
        (TaskStatus::Done, Some("nessun problema".to_string()), Some(Verdict::Issues));
    step(&mut list, &running, &board(vec![review], vec![]), 20);

    let t = only_thread(&list);
    let done = t.messages.iter().find(|m| m.event.as_ref().is_some_and(|e| e.kind == HubEventKind::TaskDone)).unwrap();
    assert_eq!(done.body, "Revisione completata: Task r (esito: problemi trovati)\nnessun problema");
    let event = done.event.as_ref().unwrap();
    assert_eq!(
        (event.summary.as_deref(), event.verdict, event.status.as_deref()),
        (Some("nessun problema"), Some(Verdict::Issues), Some("done"))
    );
}

#[test]
fn a_task_without_a_thread_gets_one_at_its_first_event() {
    let mut list = ChannelThreads::default();
    let before = board(vec![task("a", TaskStatus::Queued)], vec![]);
    step(&mut list, &before, &board(vec![task("a", TaskStatus::Running)], vec![]), 10);
    let t = only_thread(&list);
    assert_eq!((t.title.as_str(), t.task_ids.as_slice()), ("Task a", &["a".to_string()][..]));
    assert_eq!(bodies(t), ["Task avviato: Task a"]);
}

#[test]
fn deleting_a_task_is_reported_in_its_thread_only() {
    let mut list = ChannelThreads::default();
    let two = board(vec![task("a", TaskStatus::Queued), task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &board(vec![task("b", TaskStatus::Queued)], vec![]), &two, 10);
    // `a` ha il suo thread (creato a mano), `b` no: solo `a` lascia un messaggio.
    step(&mut list, &two, &board(vec![], vec![]), 20);
    let t = only_thread(&list);
    assert_eq!(bodies(t)[1], "Task eliminato: Task a");
    assert_eq!(last_event(t).kind, HubEventKind::TaskDeleted);
}

#[test]
fn a_thread_closes_when_all_its_tasks_are_concluded_and_reopens_on_a_new_event() {
    let mut list = ChannelThreads::default();
    let queued = board(vec![task("a", TaskStatus::Queued), task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &board(vec![], vec![proposal("p-1", &["a", "b"])]), &queued, 10);
    let a_done = board(vec![task("a", TaskStatus::Done), task("b", TaskStatus::Queued)], vec![]);
    step(&mut list, &queued, &a_done, 20);
    assert_eq!(only_thread(&list).state, ThreadState::Open, "b e' ancora in coda");

    // `b` eliminato: tutti conclusi, il thread si chiude con il messaggio finale.
    let concluded = board(vec![task("a", TaskStatus::Done)], vec![]);
    step(&mut list, &a_done, &concluded, 30);
    let t = only_thread(&list);
    assert_eq!(t.state, ThreadState::Closed);
    assert_eq!(bodies(t).last(), Some(&THREAD_CONCLUDED));
    assert!(t.messages.last().unwrap().event.is_none());

    // Archiviare `a` (sparito da done) non scrive nulla.
    assert!(step(&mut list, &concluded, &board(vec![], vec![]), 40).is_empty());

    // Un nuovo evento di un suo task lo riapre.
    let mut list = ChannelThreads::default();
    let running = board(vec![task("c", TaskStatus::Running)], vec![]);
    step(&mut list, &board(vec![task("c", TaskStatus::Queued)], vec![]), &running, 50);
    let done = board(vec![task("c", TaskStatus::Done)], vec![]);
    step(&mut list, &running, &done, 60);
    assert_eq!(only_thread(&list).state, ThreadState::Closed);
    let mut attention = task("c", TaskStatus::Running);
    attention.attention = true;
    let reopened = board(vec![attention], vec![]);
    step(&mut list, &done, &reopened, 70);
    let t = only_thread(&list);
    assert_eq!(t.state, ThreadState::Open);
    assert_eq!(bodies(t).last(), Some(&"Il task richiede attenzione: Task c"));
}

#[test]
fn a_failing_event_does_not_stop_the_others() {
    let mut list = ChannelThreads::default();
    let events = [
        BoardEvent::Task { change: TaskChange::Created, task: task("Bad Id", TaskStatus::Queued) },
        BoardEvent::Task { change: TaskChange::Created, task: task("ok", TaskStatus::Queued) },
    ];
    let mut n = 0;
    let next = board(vec![task("ok", TaskStatus::Queued)], vec![]);
    let touched = route(&mut list, &channel(), &next, &events, 1, &mut |prefix| {
        n += 1;
        format!("{prefix}-{n}")
    });
    // Il task con l'id non valido non lascia un thread orfano.
    let t = only_thread(&list);
    assert_eq!(t.task_ids, ["ok"]);
    assert_eq!(touched.len(), 1);
}
