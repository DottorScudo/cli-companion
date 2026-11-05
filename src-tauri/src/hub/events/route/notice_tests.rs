//! Le notifiche di sistema prodotte da `route`.

use super::*;
use crate::hub::events::diff::diff;
use crate::hub::events::fixtures::{board, proposal, task, WS};

fn route_step(list: &mut ChannelThreads, prev: &BoardSnapshot, next: &BoardSnapshot, now: i64) -> Routed {
    let mut n = 0;
    route(list, &Channel::Workspace(WS.to_string()), next, &diff(prev, next), now, &mut |prefix| {
        n += 1;
        format!("{prefix}-{now}-{n}")
    })
}

fn kinds(routed: &Routed) -> Vec<NoticeKind> {
    routed.notices.iter().map(|n| n.kind.clone()).collect()
}

fn reviewing(status: TaskStatus) -> TaskSnap {
    TaskSnap { kind: TaskKind::Review, ..task("r", status) }
}

#[test]
fn a_proposal_notifies_with_the_text_of_its_message() {
    let mut list = ChannelThreads::default();
    let out = route_step(&mut list, &board(vec![], vec![]), &board(vec![], vec![proposal("p-1", &["a"])]), 10);
    assert_eq!(kinds(&out), [NoticeKind::ProposalCreated]);
    let n = &out.notices[0];
    assert_eq!(n.thread_id, out.touched[0].id);
    assert_eq!(n.text, list.threads[0].messages[0].body);
}

#[test]
fn task_problems_and_questions_notify() {
    let mut list = ChannelThreads::default();
    let queued = board(vec![task("a", TaskStatus::Queued)], vec![]);
    route_step(&mut list, &board(vec![], vec![proposal("p-1", &["a"])]), &queued, 10);
    let running = board(vec![task("a", TaskStatus::Running)], vec![]);
    let started = route_step(&mut list, &queued, &running, 20);
    assert!(started.notices.is_empty(), "l'avvio non notifica");

    let mut waiting = task("a", TaskStatus::Running);
    waiting.attention = true;
    let attention = board(vec![waiting], vec![]);
    assert_eq!(kinds(&route_step(&mut list, &running, &attention, 30)), [NoticeKind::TaskAttention]);

    let failed = board(vec![task("a", TaskStatus::Failed)], vec![]);
    assert_eq!(kinds(&route_step(&mut list, &attention, &failed, 40)), [NoticeKind::TaskFailed]);
}

#[test]
fn a_worker_question_notifies_but_its_answer_does_not() {
    let mut list = ChannelThreads::default();
    let running = board(vec![task("a", TaskStatus::Running)], vec![]);
    route_step(&mut list, &board(vec![], vec![]), &running, 10);
    let asking = board(vec![crate::hub::events::fixtures::asking("a", "q-1", &["x", "y"])], vec![]);
    assert_eq!(kinds(&route_step(&mut list, &running, &asking, 20)), [NoticeKind::Question]);
    assert!(route_step(&mut list, &asking, &running, 30).notices.is_empty());
}

#[test]
fn a_finished_review_and_a_finished_task_differ_and_each_thread_concludes() {
    let mut list = ChannelThreads::default();
    let running = board(vec![reviewing(TaskStatus::Running), task("a", TaskStatus::Running)], vec![]);
    route_step(&mut list, &board(vec![], vec![]), &running, 10);
    let review_done = board(vec![reviewing(TaskStatus::Done), task("a", TaskStatus::Running)], vec![]);
    let out = route_step(&mut list, &running, &review_done, 20);
    assert_eq!(kinds(&out), [NoticeKind::ReviewDone, NoticeKind::ThreadConcluded]);
    let all_done = board(vec![reviewing(TaskStatus::Done), task("a", TaskStatus::Done)], vec![]);
    let out = route_step(&mut list, &review_done, &all_done, 30);
    assert_eq!(kinds(&out), [NoticeKind::TaskDone, NoticeKind::ThreadConcluded]);
    assert_eq!(out.notices[1].text, THREAD_CONCLUDED);
    assert_eq!(out.notices[0].thread_id, out.notices[1].thread_id);
}

#[test]
fn a_plan_without_a_proposal_notifies_as_such() {
    let mut list = ChannelThreads::default();
    let plan = |status| TaskSnap { kind: TaskKind::Plan, ..task("plan-x", status) };
    let running = board(vec![plan(TaskStatus::Running)], vec![]);
    route_step(&mut list, &board(vec![], vec![]), &running, 10);
    let out = route_step(&mut list, &running, &board(vec![plan(TaskStatus::Done)], vec![]), 20);
    assert_eq!(kinds(&out), [NoticeKind::PlanWithoutProposal]);
}

#[test]
fn silent_events_produce_no_notice() {
    let mut list = ChannelThreads::default();
    let pending = board(vec![], vec![proposal("p-1", &["a"])]);
    route_step(&mut list, &board(vec![], vec![]), &pending, 10);
    let accepted = board(vec![task("a", TaskStatus::Queued)], vec![]);
    assert!(route_step(&mut list, &pending, &accepted, 20).notices.is_empty(), "accettata");
    let rejected_from = board(vec![task("a", TaskStatus::Queued)], vec![proposal("p-2", &["b"])]);
    let created = route_step(&mut list, &accepted, &rejected_from, 30);
    assert_eq!(kinds(&created), [NoticeKind::ProposalCreated]);
    let rejected = route_step(&mut list, &rejected_from, &accepted, 40);
    assert!(rejected.notices.is_empty(), "scartata");
    let deleted = route_step(&mut list, &accepted, &board(vec![], vec![]), 50);
    assert!(deleted.notices.iter().all(|n| n.kind == NoticeKind::ThreadConcluded), "eliminato: {:?}", deleted.notices);
}
