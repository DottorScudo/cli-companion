//! Le richieste di modifica di un task: apertura, scheda, conferma.

use super::tests::{card, linked_thread, params, pending, task_view, view, NOW};
use super::*;
use crate::board::TaskStatus::{Failed, Queued, Running};
use crate::hub::engine::fixtures::thread;
use crate::hub::model::{FieldChange, TaskField, TaskUpdate};
use serde_json::json;

fn update_params<'a>(task_id: &'a str, patch: &'a Value) -> RequestParams<'a> {
    RequestParams { patch: Some(patch), ..params(task_id, RequestedAction::Update) }
}

fn open_update(thread: &Thread, task_id: &str, patch: &Value) -> Result<HubActionRequest, String> {
    open_request(thread, &view(), update_params(task_id, patch), "msg-1".into(), NOW)
}

/// La richiesta di cambiare il titolo di `task_id`, com'e' dopo `open_request`.
fn title_update(task_id: &str, status: &str) -> HubActionRequest {
    let update = TaskUpdate {
        changes: vec![FieldChange { field: TaskField::Title, before: Some("Old".into()), after: Some("New".into()) }],
        patch: json!({ "title": "New" }),
        base: json!({ "title": format!("Title {task_id}") }),
    };
    HubActionRequest { update: Some(update), ..pending(task_id, status, RequestedAction::Update) }
}

#[test]
fn a_failed_or_queued_task_gets_a_pending_request_with_its_diff() {
    let patch = json!({ "title": "New title", "task": "spec", "depends_on": ["r-1"] });
    let t = linked_thread(vec![]);
    for id in ["f-1", "q-1"] {
        let request = open_update(&t, id, &patch).unwrap();
        assert_eq!((request.action, request.state), (RequestedAction::Update, ActionRequestState::Pending));
        let update = request.update.as_ref().unwrap();
        let fields: Vec<_> = update.changes.iter().map(|c| c.field).collect();
        assert_eq!(fields, [TaskField::Title, TaskField::DependsOn], "{id}: la specifica e' uguale, non cambia");
        assert_eq!(update.patch, json!({ "title": "New title", "depends_on": ["r-1"] }));
        assert_eq!(request.task_title, format!("Title {id}"));
    }
}

#[test]
fn only_a_task_that_can_be_edited_gets_a_request() {
    let t = linked_thread(vec![]);
    for id in ["r-1", "d-1"] {
        let e = open_update(&t, id, &json!({ "title": "x" })).unwrap_err();
        assert!(e.ends_with("and cannot be edited now: only queued or failed tasks can be changed."), "{id}: {e}");
    }
}

#[test]
fn a_request_without_changes_or_with_a_bad_patch_is_refused_before_anything_is_written() {
    let t = linked_thread(vec![]);
    let same = open_update(&t, "f-1", &json!({ "title": "Title f-1" })).unwrap_err();
    assert!(same.contains("nothing to change"), "{same}");
    assert!(open_update(&t, "f-1", &json!({ "repo": "x" })).unwrap_err().starts_with("Only `title`"));
    let no_patch = RequestParams { patch: None, ..params("f-1", RequestedAction::Update) };
    let e = open_request(&t, &view(), no_patch, "msg-1".into(), NOW).unwrap_err();
    assert_eq!(e, "An update request needs the fields to change.");
}

#[test]
fn one_open_request_per_task_counts_updates_and_actions_alike() {
    let open = card("msg-0", "f-1", title_update("f-1", "failed"));
    let t = linked_thread(vec![open]);
    let e = open_update(&t, "f-1", &json!({ "title": "Other" })).unwrap_err();
    assert!(e.starts_with("There is already an open request"), "{e}");
    assert!(open_update(&t, "q-1", &json!({ "title": "Other" })).is_ok(), "un altro task e' libero");
}

#[test]
fn the_card_names_the_changed_fields_in_italian() {
    let patch = json!({ "title": "New", "depends_on": ["r-1"], "model": "opus" });
    let request = open_update(&linked_thread(vec![]), "f-1", &patch).unwrap();
    assert_eq!(
        texts::request_card(&request),
        "@ugo chiede di modificare «Title f-1» (titolo, dipendenze e modello): Obsoleto"
    );
    let one = open_update(&linked_thread(vec![]), "f-1", &json!({ "task": "new spec" })).unwrap();
    assert_eq!(texts::request_card(&one), "@ugo chiede di modificare «Title f-1» (specifica): Obsoleto");
    let done = HubActionRequest { state: ActionRequestState::Confirmed, ..one };
    assert_eq!(texts::outcome(&done), "Richiesta confermata: «Title f-1» modificato.");
}

#[test]
fn posting_verifies_the_normalized_patch_and_a_refusal_writes_nothing() {
    let mut list = ChannelThreads { threads: vec![linked_thread(vec![])] };
    let mut ids = 0;
    let mut next_id = |prefix: &str| {
        ids += 1;
        format!("{prefix}-{ids}")
    };
    let patch = json!({ "title": "New title", "task": "spec" });
    let seen = std::cell::RefCell::new(Vec::new());
    let verify = |p: &Value| {
        seen.borrow_mut().push(p.clone());
        Err::<(), _>("task 'f-1': dependency cycle".to_string())
    };
    let refused =
        post_request(&mut list, "t1", &view(), update_params("f-1", &patch), &mut next_id, NOW, &verify).unwrap_err();
    assert_eq!(refused, "task 'f-1': dependency cycle");
    assert_eq!(*seen.borrow(), [json!({ "title": "New title" })], "solo cio' che cambia davvero");
    assert!(list.thread("t1").unwrap().messages.is_empty());
    let (outcome, _) =
        post_request(&mut list, "t1", &view(), update_params("f-1", &patch), &mut next_id, NOW, &|_| Ok(())).unwrap();
    assert_eq!(
        (outcome["action"].as_str(), outcome["status"].as_str()),
        (Some("update"), Some("awaiting_confirmation"))
    );
    assert_eq!(list.thread("t1").unwrap().messages.len(), 1);
}

#[test]
fn a_plain_action_is_never_verified_as_an_update() {
    let mut list = ChannelThreads { threads: vec![linked_thread(vec![])] };
    let mut next_id = |prefix: &str| format!("{prefix}-1");
    let verify = |_: &Value| Err::<(), _>("non deve essere chiamata".to_string());
    let done =
        post_request(&mut list, "t1", &view(), params("f-1", RequestedAction::Delete), &mut next_id, NOW, &verify);
    assert!(done.is_ok());
}

#[test]
fn confirming_an_update_needs_the_same_state_a_task_still_editable_and_untouched_fields() {
    let request = title_update("f-1", "failed");
    let failed = task_view("f-1", Failed);
    assert_eq!(decide(&request, Some(&failed), NOW, true), Resolution::Execute(RequestedAction::Update));
    let retried = task_view("f-1", Queued);
    assert_eq!(decide(&request, Some(&retried), NOW, true), Resolution::Stale(texts::TASK_CHANGED));
    let mut locked = task_view("f-1", Failed);
    locked.can_edit = false;
    assert_eq!(decide(&request, Some(&locked), NOW, true), Resolution::Stale(texts::ACTION_NOT_ALLOWED));
    let mut edited = task_view("f-1", Failed);
    edited.task.title = "Changed meanwhile".into();
    assert_eq!(decide(&request, Some(&edited), NOW, true), Resolution::Stale(texts::TASK_EDITED));
    let running = title_update("r-1", "running");
    assert_eq!(
        decide(&running, Some(&task_view("r-1", Running)), NOW, true),
        Resolution::Stale(texts::ACTION_NOT_ALLOWED),
        "un task in corso non si modifica"
    );
}

#[test]
fn an_unlinked_thread_gets_no_update() {
    let unlinked = crate::hub::model::Thread { task_ids: vec!["q-1".into()], ..thread(vec![]) };
    let e = open_update(&unlinked, "f-1", &json!({ "title": "x" })).unwrap_err();
    assert!(e.starts_with("Task \"f-1\" is not linked to this thread"), "{e}");
}
