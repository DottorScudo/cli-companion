//! Il clic dell'utente su una board vera, scritta da una finta `ActionBoard`
//! che applica le stesse operazioni.

use super::*;
use crate::board::{AgentGate, Board, Task, TaskSpec, TaskStatus};
use crate::hub::engine::confirm::{open_request, RequestParams, REQUEST_TTL_MS};
use crate::hub::engine::fixtures::{thread, WS};
use crate::hub::model::{HubEventKind, Message, Thread};
use crate::hub::threads::NewMessage;
use std::sync::Mutex;

const NOW: i64 = 5_000_000;

fn task(id: &str, status: TaskStatus) -> Task {
    let spec = TaskSpec { id: id.into(), title: format!("Title {id}"), task: "spec".into(), ..TaskSpec::default() };
    let mut t = Task::from_spec(spec, 0);
    t.set_status(status);
    t
}

/// La board `ws-1` (`f-1` fallito, `r-1` in corso) e le azioni ricevute.
struct FakeBoard {
    board: Mutex<Board>,
    acted: Mutex<Vec<String>>,
    refuse: Option<String>,
}

impl FakeBoard {
    fn new() -> Self {
        let mut board = Board::new(WS);
        board.tasks = vec![task("f-1", TaskStatus::Failed), task("r-1", TaskStatus::Running)];
        Self { board: Mutex::new(board), acted: Mutex::default(), refuse: None }
    }

    fn view(&self) -> crate::board::BoardView {
        self.board.lock().unwrap().view_with(None, false, &AgentGate::default())
    }

    fn acted(&self) -> Vec<String> {
        self.acted.lock().unwrap().clone()
    }

    fn set_status(&self, id: &str, status: TaskStatus) {
        self.board.lock().unwrap().tasks.iter_mut().find(|t| t.id == id).unwrap().set_status(status);
    }
}

impl ActionBoard for FakeBoard {
    fn task(&self, _workspace: &str, task_id: &str) -> Option<TaskView> {
        self.view().tasks.into_iter().find(|t| t.task.id == task_id)
    }

    fn act(&self, workspace: &str, task_id: &str, action: TaskAction) -> Result<(), String> {
        if let Some(e) = &self.refuse {
            return Err(e.clone());
        }
        match action {
            TaskAction::Delete | TaskAction::DeleteCascade => {
                self.board.lock().unwrap().tasks.retain(|t| t.id != task_id)
            }
            TaskAction::Stop => self.set_status(task_id, TaskStatus::Failed),
            TaskAction::MarkDone => self.set_status(task_id, TaskStatus::Done),
            other => panic!("azione non attesa: {other:?}"),
        }
        self.acted.lock().unwrap().push(format!("{workspace} {task_id} {action:?}"));
        Ok(())
    }
}

/// Un thread con la scheda `msg-1` che chiede `action` su `task_id`.
fn list_with_request(board: &FakeBoard, task_id: &str, action: TaskAction) -> ChannelThreads {
    let base = Thread { task_ids: vec![task_id.into()], ..thread(Vec::new()) };
    let params = RequestParams { task_id, action, reason: "Obsoleto", secretary: "ugo" };
    let request = open_request(&base, &board.view(), params, "msg-1".into(), NOW).unwrap();
    let mut list = ChannelThreads { threads: vec![base] };
    let card = NewMessage {
        id: "msg-1".into(),
        author: Author::System,
        body: texts::request_card(&request),
        reply_to: None,
        event: Some(HubEvent {
            task_id: Some(task_id.into()),
            request: Some(request),
            ..HubEvent::new(HubEventKind::ActionRequest)
        }),
        origin: None,
    };
    list.add_message("t1", card, NOW).unwrap();
    list
}

fn click(list: &mut ChannelThreads, board: &FakeBoard, confirm: bool, now: i64) -> Result<Resolved, String> {
    let mut n = 1;
    let mut next_id = |prefix: &str| {
        n += 1;
        format!("{prefix}-{n}")
    };
    resolve(list, "t1", "msg-1", confirm, now, &mut next_id, board)
}

fn messages(list: &ChannelThreads) -> Vec<Message> {
    list.thread("t1").unwrap().messages.clone()
}

#[test]
fn a_confirmation_runs_the_action_once_and_writes_the_outcome_that_wakes_the_requester() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "f-1", TaskAction::Delete);
    let resolved = click(&mut list, &board, true, NOW + 1).unwrap();
    assert_eq!(board.acted(), ["ws-1 f-1 Delete"]);
    assert_eq!(resolved.request.state, ActionRequestState::Confirmed);
    assert_eq!((resolved.request.resolved_at, resolved.request.note.clone()), (Some(NOW + 1), None));
    assert_eq!(resolved.request.secretary, "ugo", "e' lui che il comando sveglia");
    let all = messages(&list);
    assert_eq!(all.len(), 2);
    let row = &all[1];
    assert_eq!(row.id, resolved.outcome_id);
    assert_eq!((row.author.clone(), row.reply_to.as_deref()), (Author::System, Some("msg-1")));
    assert_eq!(row.body, "Richiesta confermata: «Title f-1» eliminato.");
    let event = row.event.as_ref().unwrap();
    assert_eq!(event.kind, HubEventKind::ActionRequestResolved);
    assert_eq!(event.request.as_ref().unwrap().state, ActionRequestState::Confirmed);
    // La scheda mostra lo stato nuovo.
    assert_eq!(all[0].event.as_ref().unwrap().request.as_ref().unwrap().state, ActionRequestState::Confirmed);
    assert!(!resolved.touched.waiting_user);
    // Un secondo clic (doppio clic) non esegue di nuovo e non scrive.
    let again = click(&mut list, &board, true, NOW + 2).unwrap_err();
    assert_eq!(again, "Questa richiesta è già stata decisa.");
    assert_eq!(board.acted().len(), 1);
    assert_eq!(messages(&list).len(), 2);
}

#[test]
fn a_cancel_runs_nothing() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "r-1", TaskAction::Stop);
    let resolved = click(&mut list, &board, false, NOW + 1).unwrap();
    assert_eq!(resolved.request.state, ActionRequestState::Cancelled);
    assert!(board.acted().is_empty());
    assert_eq!(messages(&list)[1].body, "Richiesta annullata.");
}

#[test]
fn an_expired_request_is_returned_updated_and_runs_nothing() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "f-1", TaskAction::Delete);
    let resolved = click(&mut list, &board, true, NOW + REQUEST_TTL_MS).unwrap();
    assert_eq!(resolved.request.state, ActionRequestState::Expired);
    assert_eq!(resolved.request.note.as_deref(), Some(texts::EXPIRED_NOTE));
    assert!(board.acted().is_empty());
    assert_eq!(messages(&list)[1].body, "Richiesta scaduta: nessuna azione eseguita.");
}

#[test]
fn a_task_that_changed_makes_the_confirmation_stale_without_running_anything() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "f-1", TaskAction::Delete);
    board.set_status("f-1", TaskStatus::Queued);
    let resolved = click(&mut list, &board, true, NOW + 1).unwrap();
    assert_eq!(resolved.request.state, ActionRequestState::Stale);
    assert_eq!(resolved.request.note.as_deref(), Some(texts::TASK_CHANGED));
    assert!(board.acted().is_empty());
    assert_eq!(
        messages(&list)[1].body,
        "Richiesta non più valida. Il task ha cambiato stato. Nessuna azione eseguita."
    );
}

#[test]
fn a_task_that_is_gone_makes_the_confirmation_stale() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "f-1", TaskAction::Delete);
    board.board.lock().unwrap().tasks.retain(|t| t.id != "f-1");
    let resolved = click(&mut list, &board, true, NOW + 1).unwrap();
    assert_eq!(resolved.request.state, ActionRequestState::Stale);
    assert_eq!(resolved.request.note.as_deref(), Some(texts::TASK_GONE));
}

#[test]
fn a_refusal_of_the_board_is_a_failed_request_with_the_reason() {
    let board = FakeBoard { refuse: Some("Il task è già in corso.".into()), ..FakeBoard::new() };
    let mut list = list_with_request(&FakeBoard::new(), "f-1", TaskAction::Delete);
    let resolved = click(&mut list, &board, true, NOW + 1).unwrap();
    assert_eq!(resolved.request.state, ActionRequestState::Failed);
    assert_eq!(resolved.request.note.as_deref(), Some("Il task è già in corso."));
    assert_eq!(
        messages(&list)[1].body,
        "Richiesta confermata, ma la board non ha eseguito l'azione: Il task è già in corso."
    );
}

#[test]
fn a_message_that_is_not_a_request_is_not_found() {
    let board = FakeBoard::new();
    let mut list = list_with_request(&board, "f-1", TaskAction::Delete);
    let mut next_id = |prefix: &str| format!("{prefix}-9");
    assert_eq!(resolve(&mut list, "t1", "nope", true, NOW, &mut next_id, &board).unwrap_err(), "richiesta non trovata");
    assert_eq!(resolve(&mut list, "zz", "msg-1", true, NOW, &mut next_id, &board).unwrap_err(), THREAD_NOT_FOUND);
    assert!(board.acted().is_empty());
}
