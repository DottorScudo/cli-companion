//! Le richieste di azione in attesa quando il task cambia sulla board.

use super::*;
use crate::hub::events::diff::diff;
use crate::hub::events::fixtures::{board, task, WS};
use crate::hub::model::{ActionRequestState, HubActionRequest};

fn route_step(list: &mut ChannelThreads, prev: &BoardSnapshot, next: &BoardSnapshot, now: i64) -> Routed {
    let mut n = 0;
    route(list, &Channel::Workspace(WS.to_string()), next, &diff(prev, next), now, &mut |prefix| {
        n += 1;
        format!("{prefix}-{now}-{n}")
    })
}

/// Un thread con il task `a` fallito e la richiesta `req-1` di eliminarlo.
fn thread_with_request(state: ActionRequestState) -> (ChannelThreads, BoardSnapshot) {
    let failed = board(vec![task("a", TaskStatus::Failed)], vec![]);
    let mut list = ChannelThreads::default();
    route_step(&mut list, &board(vec![], vec![]), &failed, 10);
    let thread_id = list.threads[0].id.clone();
    let request = HubActionRequest {
        id: "req-1".into(),
        action: crate::hub::model::RequestedAction::Delete,
        reason: "Obsoleto".into(),
        secretary: "ugo".into(),
        task_title: "Task a".into(),
        task_status: "failed".into(),
        dependents: Vec::new(),
        update: None,
        expires_at: 10 + 60_000,
        state,
        resolved_at: None,
        note: None,
    };
    let card = NewMessage {
        id: "req-1".into(),
        author: Author::System,
        body: "@ugo chiede di eliminare «Task a»: Obsoleto".into(),
        reply_to: None,
        event: Some(HubEvent {
            task_id: Some("a".into()),
            request: Some(request),
            ..HubEvent::new(HubEventKind::ActionRequest)
        }),
        origin: None,
    };
    list.add_message(&thread_id, card, 11).unwrap();
    (list, failed)
}

fn request_state(list: &ChannelThreads) -> HubActionRequest {
    let card = list.threads[0].messages.iter().find(|m| m.id == "req-1").unwrap();
    card.event.as_ref().unwrap().request.clone().unwrap()
}

#[test]
fn a_task_that_changes_state_stales_its_pending_request() {
    let (mut list, failed) = thread_with_request(ActionRequestState::Pending);
    let restarted = board(vec![task("a", TaskStatus::Running)], vec![]);
    let out = route_step(&mut list, &failed, &restarted, 20);
    assert_eq!(out.touched.len(), 1);
    let request = request_state(&list);
    assert_eq!(request.state, ActionRequestState::Stale);
    assert_eq!(request.resolved_at, Some(20));
    assert!(request.note.is_some());
    assert!(!out.touched[0].waiting_user, "il task e' ripartito e la richiesta non fa aspettare nessuno");
}

#[test]
fn a_deleted_task_stales_its_pending_request() {
    let (mut list, failed) = thread_with_request(ActionRequestState::Pending);
    route_step(&mut list, &failed, &board(vec![], vec![]), 20);
    assert_eq!(request_state(&list).state, ActionRequestState::Stale);
}

#[test]
fn a_confirmed_request_is_never_touched_by_the_events_its_action_causes() {
    let (mut list, failed) = thread_with_request(ActionRequestState::Confirmed);
    route_step(&mut list, &failed, &board(vec![], vec![]), 20);
    let request = request_state(&list);
    assert_eq!((request.state, request.resolved_at, request.note), (ActionRequestState::Confirmed, None, None));
}

#[test]
fn an_event_of_another_task_leaves_the_request_alone() {
    let (mut list, failed) = thread_with_request(ActionRequestState::Pending);
    let both = board(vec![task("a", TaskStatus::Failed), task("b", TaskStatus::Queued)], vec![]);
    route_step(&mut list, &failed, &both, 20);
    assert_eq!(request_state(&list).state, ActionRequestState::Pending);
}

#[test]
fn request_rows_neither_wait_for_the_user_nor_count_as_the_task_events() {
    let (list, _) = thread_with_request(ActionRequestState::Pending);
    let thread = &list.threads[0];
    // Il task fallito aspetta l'utente; la richiesta non cambia questo.
    assert!(crate::hub::waiting::waiting_user(&thread.messages));
    let card = thread.messages.last().unwrap();
    assert_eq!(card.id, "req-1");
    assert!(!crate::hub::waiting::waiting_user(std::slice::from_ref(card)));
}
