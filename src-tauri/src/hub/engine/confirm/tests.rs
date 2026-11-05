use super::*;
use crate::board::{AgentGate, Board, Task, TaskSpec, TaskStatus};
use crate::hub::channel::Channel;
use crate::hub::engine::fixtures::{thread, WS};
use crate::hub::model::{Author, MessageKind};
use TaskStatus::{Done, Failed, Queued, Running};

pub(super) const NOW: i64 = 1_000_000;

pub(super) fn task(id: &str, status: TaskStatus, depends_on: &[&str]) -> Task {
    let spec = TaskSpec {
        id: id.into(),
        title: format!("Title {id}"),
        task: "spec".into(),
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        ..TaskSpec::default()
    };
    let mut t = Task::from_spec(spec, 0);
    t.set_status(status);
    t
}

/// `f-1` fallito, `r-1` in corso, `q-1` in coda con `q-2` che ne dipende, `d-1` fatto.
pub(super) fn board() -> Board {
    let mut board = Board::new(WS);
    board.tasks = vec![
        task("f-1", Failed, &[]),
        task("r-1", Running, &[]),
        task("q-1", Queued, &[]),
        task("q-2", Queued, &["q-1"]),
        task("d-1", Done, &[]),
    ];
    board
}

pub(super) fn view_of(board: &Board) -> BoardView {
    board.view_with(None, false, &AgentGate::default())
}

pub(super) fn view() -> BoardView {
    view_of(&board())
}

/// Il thread del workspace di prova, con tutti i task della board collegati.
pub(super) fn linked_thread(messages: Vec<Message>) -> Thread {
    Thread { task_ids: ["f-1", "r-1", "q-1", "q-2", "d-1"].map(String::from).to_vec(), ..thread(messages) }
}

pub(super) fn params<'a>(task_id: &'a str, action: RequestedAction) -> RequestParams<'a> {
    RequestParams { task_id, action, patch: None, reason: "Obsoleto", secretary: "ugo" }
}

pub(super) fn open(thread: &Thread, task_id: &str, action: RequestedAction) -> Result<HubActionRequest, String> {
    open_request(thread, &view(), params(task_id, action), "msg-1".into(), NOW)
}

/// La scheda di una richiesta del task `task_id`.
pub(super) fn card(id: &str, task_id: &str, request: HubActionRequest) -> Message {
    Message {
        id: id.into(),
        author: Author::System,
        kind: MessageKind::Event,
        body: "scheda".into(),
        reply_to: None,
        event: Some(HubEvent {
            task_id: Some(task_id.into()),
            request: Some(HubActionRequest { id: id.into(), ..request }),
            ..HubEvent::new(HubEventKind::ActionRequest)
        }),
        attachments: Vec::new(),
        origin: None,
        created_at: 0,
    }
}

pub(super) fn pending(task_id: &str, status: &str, action: RequestedAction) -> HubActionRequest {
    HubActionRequest {
        id: "msg-0".into(),
        action,
        reason: "Obsoleto".into(),
        secretary: "ugo".into(),
        task_title: format!("Title {task_id}"),
        task_status: status.into(),
        dependents: Vec::new(),
        update: None,
        expires_at: NOW + REQUEST_TTL_MS,
        state: ActionRequestState::Pending,
        resolved_at: None,
        note: None,
    }
}

pub(super) fn task_view(id: &str, status: TaskStatus) -> TaskView {
    let mut b = board();
    b.tasks.iter_mut().find(|t| t.id == id).unwrap().set_status(status);
    view_of(&b).tasks.into_iter().find(|t| t.task.id == id).unwrap()
}

#[test]
fn a_valid_request_is_pending_for_thirty_minutes_and_remembers_the_task_state() {
    let request = open(&linked_thread(vec![]), "f-1", RequestedAction::Delete).unwrap();
    assert_eq!(request.id, "msg-1");
    assert_eq!((request.action, request.state), (RequestedAction::Delete, ActionRequestState::Pending));
    assert_eq!((request.task_title.as_str(), request.task_status.as_str()), ("Title f-1", "failed"));
    assert_eq!((request.secretary.as_str(), request.reason.as_str()), ("ugo", "Obsoleto"));
    assert_eq!(request.expires_at, NOW + 30 * 60 * 1000);
    assert_eq!((request.resolved_at, request.note, request.dependents.len()), (None, None, 0));
}

#[test]
fn the_four_requestable_actions_are_accepted_when_the_task_allows_them() {
    let t = linked_thread(vec![]);
    assert!(open(&t, "r-1", RequestedAction::Stop).is_ok());
    assert!(open(&t, "f-1", RequestedAction::MarkDone).is_ok());
    assert!(open(&t, "q-2", RequestedAction::Delete).is_ok());
    let cascade = open(&t, "q-1", RequestedAction::DeleteCascade).unwrap();
    assert_eq!(cascade.dependents, ["Title q-2"], "i titoli dei task che la cascata toglie");
}

#[test]
fn an_action_the_task_does_not_allow_is_refused_with_its_state() {
    let t = linked_thread(vec![]);
    let e = open(&t, "d-1", RequestedAction::Delete).unwrap_err();
    assert_eq!(e, "Task \"d-1\" (\"Title d-1\") is done and `delete` is not allowed on it now.");
    assert!(open(&t, "q-1", RequestedAction::Stop).is_err());
    assert!(open(&t, "r-1", RequestedAction::MarkDone).is_err());
}

#[test]
fn a_task_not_linked_to_the_thread_or_not_on_the_board_is_refused() {
    let unlinked = Thread { task_ids: vec!["q-1".into()], ..thread(vec![]) };
    let e = open(&unlinked, "f-1", RequestedAction::Delete).unwrap_err();
    assert!(e.starts_with("Task \"f-1\" is not linked to this thread"), "{e}");
    let ghost = Thread { task_ids: vec!["x-9".into()], ..thread(vec![]) };
    let e = open(&ghost, "x-9", RequestedAction::Delete).unwrap_err();
    assert!(e.starts_with("No task with id \"x-9\" on the board"), "{e}");
}

#[test]
fn only_a_thread_of_the_workspace_of_the_board_can_ask() {
    let general = Thread { channel: Channel::General, ..linked_thread(vec![]) };
    assert_eq!(open(&general, "f-1", RequestedAction::Delete).unwrap_err(), NOT_A_WORKSPACE_THREAD);
    let other = Thread { channel: Channel::Workspace("ws-2".into()), ..linked_thread(vec![]) };
    assert_eq!(open(&other, "f-1", RequestedAction::Delete).unwrap_err(), NOT_A_WORKSPACE_THREAD);
}

#[test]
fn a_cascade_over_running_tasks_is_refused() {
    let mut v = view();
    let blocked = v.tasks.iter_mut().find(|t| t.task.id == "q-1").unwrap();
    blocked.cascade.running = vec!["r-1".into()];
    let e =
        open_request(&linked_thread(vec![]), &v, params("q-1", RequestedAction::DeleteCascade), "msg-1".into(), NOW)
            .unwrap_err();
    assert!(e.contains("would remove running tasks (r-1)"), "{e}");
}

#[test]
fn only_one_request_per_task_can_be_open() {
    let first = open(&linked_thread(vec![]), "f-1", RequestedAction::Delete).unwrap();
    let t = linked_thread(vec![card("msg-1", "f-1", first.clone())]);
    let e = open(&t, "f-1", RequestedAction::MarkDone).unwrap_err();
    assert!(e.starts_with("There is already an open request for task \"f-1\""), "{e}");
    assert!(open(&t, "r-1", RequestedAction::Stop).is_ok(), "un altro task e' libero");
    // Scaduta (anche solo al limite) o decisa: se ne puo' chiedere un'altra.
    let at_expiry =
        open_request(&t, &view(), params("f-1", RequestedAction::MarkDone), "msg-2".into(), first.expires_at);
    assert!(at_expiry.is_ok(), "alla scadenza la richiesta non e' piu' aperta");
    for state in [ActionRequestState::Confirmed, ActionRequestState::Cancelled, ActionRequestState::Stale] {
        let done = HubActionRequest { state, ..first.clone() };
        assert!(open(&linked_thread(vec![card("msg-1", "f-1", done)]), "f-1", RequestedAction::MarkDone).is_ok());
    }
}

#[test]
fn the_confirmation_runs_only_while_everything_is_as_requested() {
    let request = pending("f-1", "failed", RequestedAction::Delete);
    let failed = task_view("f-1", Failed);
    assert_eq!(decide(&request, Some(&failed), NOW, true), Resolution::Execute(RequestedAction::Delete));
    // Il limite: l'ultimo millisecondo vale, la scadenza no.
    assert_eq!(
        decide(&request, Some(&failed), request.expires_at - 1, true),
        Resolution::Execute(RequestedAction::Delete)
    );
    assert_eq!(decide(&request, Some(&failed), request.expires_at, true), Resolution::Expired);
    assert_eq!(decide(&request, Some(&failed), request.expires_at + 1, true), Resolution::Expired);
}

#[test]
fn a_task_that_changed_or_vanished_makes_the_request_stale() {
    let request = pending("f-1", "failed", RequestedAction::Delete);
    assert_eq!(decide(&request, None, NOW, true), Resolution::Stale(texts::TASK_GONE));
    let retried = task_view("f-1", Queued);
    assert_eq!(decide(&request, Some(&retried), NOW, true), Resolution::Stale(texts::TASK_CHANGED));
    // Stesso stato ma l'azione non e' piu' ammessa.
    let mut same = task_view("f-1", Failed);
    same.allowed_actions.clear();
    assert_eq!(decide(&request, Some(&same), NOW, true), Resolution::Stale(texts::ACTION_NOT_ALLOWED));
    let cascade = pending("q-1", "queued", RequestedAction::DeleteCascade);
    let mut blocked = task_view("q-1", Queued);
    blocked.cascade.running = vec!["r-1".into()];
    assert_eq!(decide(&cascade, Some(&blocked), NOW, true), Resolution::Stale(texts::ACTION_NOT_ALLOWED));
}

#[test]
fn cancel_works_on_a_pending_request_even_if_expired_and_never_needs_the_task() {
    let request = pending("f-1", "failed", RequestedAction::Delete);
    assert_eq!(decide(&request, None, NOW, false), Resolution::Cancel);
    assert_eq!(decide(&request, None, request.expires_at + 1, false), Resolution::Cancel);
}

#[test]
fn a_request_already_decided_cannot_be_decided_again() {
    let failed = task_view("f-1", Failed);
    for state in [
        ActionRequestState::Confirmed,
        ActionRequestState::Cancelled,
        ActionRequestState::Expired,
        ActionRequestState::Stale,
        ActionRequestState::Failed,
    ] {
        let request = HubActionRequest { state, ..pending("f-1", "failed", RequestedAction::Delete) };
        for confirm in [true, false] {
            assert_eq!(decide(&request, Some(&failed), NOW, confirm), Resolution::AlreadyResolved, "{state:?}");
        }
    }
}

fn list_with(thread: Thread) -> ChannelThreads {
    ChannelThreads { threads: vec![thread] }
}

fn state_of(list: &ChannelThreads, message: &str) -> (ActionRequestState, Option<String>) {
    let m = list.thread("t1").unwrap().messages.iter().find(|m| m.id == message).unwrap();
    let request = m.event.as_ref().unwrap().request.as_ref().unwrap();
    (request.state, request.note.clone())
}

#[test]
fn a_task_that_changes_state_stales_only_its_pending_requests() {
    let confirmed =
        HubActionRequest { state: ActionRequestState::Confirmed, ..pending("f-1", "failed", RequestedAction::Delete) };
    let mut list = list_with(linked_thread(vec![
        card("a", "f-1", pending("f-1", "failed", RequestedAction::Delete)),
        card("b", "f-1", confirmed),
        card("c", "r-1", pending("r-1", "running", RequestedAction::Stop)),
    ]));
    assert_eq!(invalidate_for_task(&mut list, "t1", "f-1", Some("queued"), NOW), Ok(1));
    assert_eq!(state_of(&list, "a"), (ActionRequestState::Stale, Some(texts::TASK_CHANGED.into())));
    assert_eq!(state_of(&list, "b").0, ActionRequestState::Confirmed, "una decisa non si tocca");
    assert_eq!(state_of(&list, "c").0, ActionRequestState::Pending, "un altro task non si tocca");
    let request = list.thread("t1").unwrap().messages[0].event.as_ref().unwrap().request.as_ref().unwrap();
    assert_eq!(request.resolved_at, Some(NOW));
}

#[test]
fn an_event_that_leaves_the_status_alone_keeps_the_request_and_a_deleted_task_stales_it() {
    let mut list = list_with(linked_thread(vec![card("a", "f-1", pending("f-1", "failed", RequestedAction::Delete))]));
    assert_eq!(invalidate_for_task(&mut list, "t1", "f-1", Some("failed"), NOW), Ok(0));
    assert_eq!(state_of(&list, "a").0, ActionRequestState::Pending);
    assert_eq!(invalidate_for_task(&mut list, "t1", "f-1", None, NOW), Ok(1));
    assert_eq!(state_of(&list, "a"), (ActionRequestState::Stale, Some(texts::TASK_GONE.into())));
    assert_eq!(invalidate_for_task(&mut list, "t1", "f-1", None, NOW), Ok(0), "una volta sola");
}

#[test]
fn the_card_and_the_outcome_rows_are_in_italian() {
    let request = HubActionRequest {
        dependents: vec!["Title q-2".into()],
        ..pending("q-1", "queued", RequestedAction::DeleteCascade)
    };
    assert_eq!(
        texts::request_card(&pending("f-1", "failed", RequestedAction::Delete)),
        "@ugo chiede di eliminare «Title f-1»: Obsoleto"
    );
    assert_eq!(
        texts::request_card(&request),
        "@ugo chiede di eliminare «Title q-1» e il task che ne dipende: Obsoleto"
    );
    let many = HubActionRequest { dependents: vec!["A".into(), "B".into()], ..request.clone() };
    assert!(texts::request_card(&many).contains("e i 2 task che ne dipendono"));
    assert_eq!(
        texts::request_card(&pending("r-1", "running", RequestedAction::Stop)),
        "@ugo chiede di fermare «Title r-1»: Obsoleto"
    );
    let outcome = |state, note: Option<&str>| {
        texts::outcome(&HubActionRequest {
            state,
            note: note.map(String::from),
            ..pending("f-1", "failed", RequestedAction::Delete)
        })
    };
    assert_eq!(outcome(ActionRequestState::Confirmed, None), "Richiesta confermata: «Title f-1» eliminato.");
    assert_eq!(outcome(ActionRequestState::Cancelled, None), "Richiesta annullata.");
    assert_eq!(outcome(ActionRequestState::Expired, None), "Richiesta scaduta: nessuna azione eseguita.");
    assert_eq!(
        outcome(ActionRequestState::Stale, Some(texts::TASK_CHANGED)),
        "Richiesta non più valida. Il task ha cambiato stato. Nessuna azione eseguita."
    );
    assert_eq!(
        outcome(ActionRequestState::Failed, Some("Il task è già stato eliminato.")),
        "Richiesta confermata, ma la board non ha eseguito l'azione: Il task è già stato eliminato."
    );
}

#[test]
fn posting_a_request_writes_a_system_card_that_names_the_secretary() {
    let mut list = list_with(linked_thread(vec![]));
    let mut ids = 0;
    let mut next_id = |prefix: &str| {
        ids += 1;
        format!("{prefix}-{ids}")
    };
    let (outcome, touched) =
        post_request(&mut list, "t1", &view(), params("f-1", RequestedAction::Delete), &mut next_id, NOW, &|_| Ok(()))
            .unwrap();
    assert_eq!(touched.message_count, 1);
    assert!(!touched.waiting_user, "una richiesta non e' qualcosa che il thread deve all'utente");
    let message = &list.thread("t1").unwrap().messages[0];
    assert_eq!((message.id.as_str(), &message.author, message.kind), ("msg-1", &Author::System, MessageKind::Event));
    assert_eq!(message.body, "@ugo chiede di eliminare «Title f-1»: Obsoleto");
    let event = message.event.as_ref().unwrap();
    assert_eq!(
        (event.kind, event.task_id.as_deref(), event.workspace.as_deref()),
        (HubEventKind::ActionRequest, Some("f-1"), Some(WS))
    );
    assert_eq!(event.request.as_ref().unwrap().id, "msg-1");
    assert_eq!(outcome["request_id"], "msg-1");
    assert_eq!(outcome["status"], "awaiting_confirmation");
    assert_eq!(outcome["expires_in_minutes"], 30);
    assert_eq!(outcome["action"], "delete");
    // Una seconda richiesta per lo stesso task non entra e non scrive nulla.
    let again = post_request(
        &mut list,
        "t1",
        &view(),
        params("f-1", RequestedAction::MarkDone),
        &mut next_id,
        NOW,
        &|_| Ok(()),
    );
    assert!(again.unwrap_err().starts_with("There is already an open request"));
    assert_eq!(list.thread("t1").unwrap().messages.len(), 1);
}
