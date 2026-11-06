//! I testi lunghi dei task negli eventi di `route`: mai tagliati in silenzio.

use super::*;
use crate::hub::events::diff::diff;
use crate::hub::events::fixtures::{board, task, WS};
use crate::validate::clip_with_ellipsis;

const THREAD: &str = "thr-plan";
const PLAN: &str = "plan-x";

fn route_step(list: &mut ChannelThreads, prev: &BoardSnapshot, next: &BoardSnapshot, now: i64) -> Routed {
    let mut n = 0;
    route(list, &Channel::Workspace(WS.to_string()), next, &diff(prev, next), now, &mut |prefix| {
        n += 1;
        format!("{prefix}-{now}-{n}")
    })
}

/// Il thread del task `plan`, con il titolare "giorgio".
fn planning_thread() -> ChannelThreads {
    let mut list = ChannelThreads::default();
    list.create_thread(&Channel::Workspace(WS.to_string()), THREAD.to_string(), "Aggiungi X", 1).unwrap();
    list.link_tasks(THREAD, &[PLAN.to_string()], 1).unwrap();
    list.set_owner(THREAD, "giorgio", 1).unwrap();
    list
}

fn plan(status: TaskStatus) -> TaskSnap {
    TaskSnap { kind: TaskKind::Plan, ..task(PLAN, status) }
}

/// Il testo della parte senza l'etichetta "(parte i/n)".
fn without_label(body: &str) -> &str {
    body.split_once('\n').filter(|(label, _)| label.starts_with("(parte ")).map_or(body, |(_, rest)| rest)
}

#[test]
fn a_plan_without_proposal_reports_all_its_questions_in_consecutive_parts() {
    let mut list = planning_thread();
    let running = board(vec![plan(TaskStatus::Running)], vec![]);
    let questions: Vec<String> = (1..=600).map(|i| format!("{i}. Quale database per il modulo {i}?")).collect();
    let report = format!("Questions for the user:\n\n{}", questions.join("\n\n"));
    assert!(report.chars().count() >= 20_000);
    let mut finished = plan(TaskStatus::Done);
    finished.summary = Some(clip_with_ellipsis(&report, 1000));
    finished.summary_chars = Some(report.chars().count());
    finished.plan_report = Some(report.clone());
    let out = route_step(&mut list, &running, &board(vec![finished], vec![]), 10);

    let messages = &list.thread(THREAD).unwrap().messages;
    assert!(messages.len() > 1);
    let rebuilt: String = messages.iter().map(|m| without_label(&m.body)).collect();
    assert_eq!(rebuilt, format!("Pianificazione conclusa senza proposta: Task plan-x\n{report}"));
    let event = messages[0].event.as_ref().expect("la prima parte porta l'evento");
    assert_eq!((event.kind, event.summary_chars), (HubEventKind::TaskDone, Some(report.chars().count())));
    assert!(messages[1..].iter().all(|m| m.event.is_none()));
    assert_eq!(out.notices.len(), 1, "una sola notifica");
    assert_eq!(out.notices[0].message_id, messages[0].id);
    let last = messages.last().unwrap().id.clone();
    assert_eq!(out.wakes, [EventWake { thread_id: THREAD.into(), secretary: "giorgio".into(), trigger: last }]);
}

#[test]
fn a_concluded_task_event_carries_the_whole_lengths_only_when_shortened() {
    let mut list = ChannelThreads::default();
    let running = board(vec![task("a", TaskStatus::Running)], vec![]);
    route_step(&mut list, &board(vec![], vec![]), &running, 10);
    let mut failed = task("a", TaskStatus::Failed);
    (failed.summary, failed.summary_chars) = (Some(format!("{}…", "s".repeat(1000))), Some(20_000));
    (failed.note, failed.note_chars) = (Some(format!("{}…", "n".repeat(1000))), Some(1500));
    let failed_board = board(vec![failed], vec![]);
    route_step(&mut list, &running, &failed_board, 20);
    let mut done = task("a", TaskStatus::Done);
    done.summary = Some("tutto ok".to_string());
    route_step(&mut list, &failed_board, &board(vec![done], vec![]), 30);

    let events: Vec<&HubEvent> = list.threads[0].messages.iter().filter_map(|m| m.event.as_ref()).collect();
    let failed = events.iter().find(|e| e.kind == HubEventKind::TaskFailed).unwrap();
    assert_eq!((failed.summary_chars, failed.note_chars), (Some(20_000), Some(1500)));
    let done = events.iter().find(|e| e.kind == HubEventKind::TaskDone).unwrap();
    assert_eq!((done.summary.as_deref(), done.summary_chars, done.note_chars), (Some("tutto ok"), None, None));
    let json = serde_json::to_value(done).unwrap();
    assert!(json.get("summary_chars").is_none() && json.get("note_chars").is_none(), "{json}");
}
