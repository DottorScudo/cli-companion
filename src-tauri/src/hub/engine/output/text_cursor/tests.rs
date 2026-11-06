use super::super::task_details::pieces::size;
use super::*;
use crate::board::{
    build_report, AgentGate, Board, Finished, ReportFields, SpecContext, TaskSpec, TaskState, TaskView,
};
use crate::hub::engine::chat::ToolCall;
use crate::hub::engine::context::{turn_message, Trigger};
use crate::hub::engine::fixtures::{self, msg, names, secretary, WS};
use crate::hub::engine::output::{task_details, thread};
use crate::hub::engine::sessions::WakeCause;
use crate::hub::engine::thread_page::ThreadPage;
use crate::hub::engine::tools::{self, Invocation};
use crate::hub::engine::turn::TOOL_RESULT_MAX;
use crate::hub::model::{Author, HubEventKind, MessageKind, Thread};
use crate::validate::clip_with_ellipsis;

fn marked(event: &HubEvent) -> Value {
    let mut value = serde_json::to_value(event).unwrap();
    mark_shortened(event, &mut value);
    value
}

#[test]
fn a_shortened_summary_and_note_say_how_long_they_are_and_how_to_read_them() {
    let event = HubEvent {
        workspace: Some("ws-2".into()),
        summary: Some("s…".into()),
        summary_chars: Some(20_000),
        note: Some("n…".into()),
        note_chars: Some(1500),
        ..HubEvent::of_task(HubEventKind::TaskFailed, "t-7")
    };
    let v = marked(&event);
    assert!(v.get("summary_chars").is_none() && v.get("note_chars").is_none(), "{v}");
    assert_eq!(
        v["summary_truncated"],
        json!({ "total_chars": 20_000, "read_with": { "tool": "task_details", "arguments": {
            "task_id": "t-7", "workspace": "ws-2", "field": "report.summary", "offset": 0
        } } })
    );
    assert_eq!(v["note_truncated"]["total_chars"], 1500);
    assert_eq!(v["note_truncated"]["read_with"]["arguments"]["field"], "note");
}

#[test]
fn whole_texts_have_no_marker() {
    let event = HubEvent { summary: Some("ok".into()), ..HubEvent::of_task(HubEventKind::TaskDone, "t-1") };
    let v = marked(&event);
    assert!(MARKER_KEYS.iter().all(|key| v.get(key).is_none()), "{v}");
}

#[test]
fn without_a_task_or_a_workspace_the_marker_keeps_what_it_knows() {
    let event = HubEvent { summary_chars: Some(5000), ..HubEvent::of_proposal(HubEventKind::ProposalCreated, "p") };
    assert_eq!(marked(&event)["summary_truncated"], json!({ "total_chars": 5000 }));
    let local = HubEvent { note_chars: Some(5000), ..HubEvent::of_task(HubEventKind::TaskFailed, "t-1") };
    let arguments = &marked(&local)["note_truncated"]["read_with"]["arguments"];
    assert_eq!(*arguments, json!({ "task_id": "t-1", "field": "note", "offset": 0 }));
}

// --- Dall'evento al testo intero, come lo legge il modello ---

/// Caratteri del riassunto dell'evento, come in `events::snapshot`.
const EVENT_KEPT: usize = 1000;
const SUMMARY_CHARS: usize = 20_000;

/// Un riepilogo lungo con a capo, virgolette e caratteri multibyte.
fn long_summary() -> String {
    "Caso «colpito» \"A\"\nprova: ok 🙂 ".chars().cycle().take(SUMMARY_CHARS).collect()
}

fn done_task(summary: &str) -> TaskView {
    let mut board = Board::new(WS);
    let spec = TaskSpec { id: "t-1".into(), title: "Analisi".into(), task: "Analizza".into(), ..TaskSpec::default() };
    board.create_task(spec, SpecContext::default(), 1_700_000_000_000).unwrap();
    let report = build_report("completed", summary, ReportFields::default(), None);
    let done = Finished { started_at: None, finished_at: 1_700_000_001_000, report: Some(report) };
    board.task_mut("t-1").unwrap().set_state(TaskState::Done(done));
    board.view_with(None, false, &AgentGate::default()).tasks.remove(0)
}

/// Il thread con l'evento `e1` del task concluso, accorciato come lo fa la board.
fn thread_with_done_event(summary: &str) -> Thread {
    let mut event = msg("e1", Author::System, "Task finito: Analisi", 1_700_000_002_000);
    event.kind = MessageKind::Event;
    event.event = Some(HubEvent {
        workspace: Some(WS.into()),
        status: Some("done".into()),
        summary: Some(clip_with_ellipsis(summary, EVENT_KEPT)),
        summary_chars: Some(summary.chars().count()),
        ..HubEvent::of_task(HubEventKind::TaskDone, "t-1")
    });
    fixtures::thread(vec![msg("m0", Author::User, "analizza", 0), event])
}

/// Il JSON dell'evento dentro il messaggio che apre il turno.
fn opening_event(t: &Thread) -> Value {
    let trigger = Trigger { id: "e1", cause: WakeCause::BoardEvent, proposal: None };
    let text = turn_message(t, &trigger, None, "mario", &names()).unwrap();
    let line = text.lines().skip_while(|l| !l.starts_with("A board event")).nth(1).unwrap();
    serde_json::from_str::<Value>(line).unwrap()["event"].clone()
}

/// Il testo che il modello ricostruisce seguendo `read_with` con il tool vero
/// (validazione degli argomenti compresa) finche' `next_offset` manca.
fn follow(cursor: &Value, v: &TaskView) -> String {
    assert_eq!(cursor["read_with"]["tool"], "task_details");
    let mut arguments = cursor["read_with"]["arguments"].clone();
    let mut text = String::new();
    loop {
        let call = ToolCall { id: "c".into(), name: "task_details".into(), arguments: arguments.to_string() };
        let Ok(Invocation::TaskDetails { workspace, task_id, page }) = tools::parse(&call, &secretary("mario")) else {
            panic!("{arguments}");
        };
        assert_eq!((workspace.as_deref(), task_id.as_str()), (Some(WS), "t-1"));
        let part = task_details(v, WS, &names(), page.as_ref()).unwrap();
        assert!(size(&part) <= TOOL_RESULT_MAX);
        assert_eq!(part["total_chars"].as_u64(), cursor["total_chars"].as_u64());
        text.push_str(part["content"].as_str().unwrap());
        match part["next_offset"].as_u64() {
            Some(next) => arguments["offset"] = Value::from(next),
            None => return text,
        }
    }
}

#[test]
fn a_long_report_summary_is_reachable_whole_from_the_opening_event_and_from_thread_read() {
    let summary = long_summary();
    let (v, t) = (done_task(&summary), thread_with_done_event(&summary));
    let opening = opening_event(&t);
    let page = thread(&t, &names(), &ThreadPage::Latest).unwrap();
    let in_page = &page["messages"][1]["event"];
    assert!(size(&page) <= TOOL_RESULT_MAX);
    for event in [&opening, in_page] {
        let cursor = &event["summary_truncated"];
        assert_eq!(cursor["total_chars"].as_u64(), Some(SUMMARY_CHARS as u64), "{event}");
        assert_eq!(
            cursor["read_with"]["arguments"],
            json!({ "task_id": "t-1", "workspace": WS, "field": "report.summary", "offset": 0 })
        );
        assert!(event.get("summary_chars").is_none(), "{event}");
        assert_eq!(follow(cursor, &v), summary);
    }
}

#[test]
fn an_event_reduced_to_its_ids_keeps_the_cursor() {
    let summary = long_summary();
    let mut t = thread_with_done_event(&summary);
    t.messages[1].body = "x".repeat(20_000);
    if let Some(event) = t.messages[1].event.as_mut() {
        event.note = Some("n".repeat(EVENT_KEPT) + "…");
        event.note_chars = Some(3000);
        event.question =
            Some(crate::hub::model::HubQuestion { id: "q-1".into(), text: "q".repeat(4000), options: Vec::new() });
    }
    let page = thread(&t, &names(), &ThreadPage::Latest).unwrap();
    let m = &page["messages"][0];
    assert_eq!(m["event_truncated"], true, "{m}");
    assert_eq!(m["event"]["summary_truncated"]["total_chars"].as_u64(), Some(SUMMARY_CHARS as u64));
    assert_eq!(m["event"]["note_truncated"]["read_with"]["arguments"]["field"], "note");
    assert!(m["event"].get("summary").is_none() && m["event"].get("note").is_none());
    assert_eq!(follow(&m["event"]["summary_truncated"], &done_task(&summary)), summary);
}
