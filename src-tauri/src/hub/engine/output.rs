//! Come il modello vede board e thread (puro): JSON compatto, con i testi
//! tagliati. Gli eventi della board escono con i loro dati strutturati, non
//! solo con il testo italiano del messaggio. Lo usano i tool (`board_status`,
//! `thread_read`) e il contesto del risveglio (`context`).

use super::workspace_names::WorkspaceNames;
use crate::board::BoardView;
use crate::hub::channel::Channel;
use crate::hub::model::{Message, Thread};
use crate::validate::clip_with_ellipsis;
use serde_json::{json, Map, Value};

/// Task riportati da `board_status`: oltre, si conta solo quanti ne mancano.
pub const BOARD_TASKS_MAX: usize = 40;
/// Messaggi riportati da `thread_read`.
pub const THREAD_READ_MESSAGES: usize = 20;
/// Caratteri tenuti del testo di un messaggio.
const BODY_KEPT: usize = 1000;

/// La board in righe compatte (`BoardView::to_list_json`: conteggi, task con
/// stato, attenzione, attesa e riassunto), al massimo `BOARD_TASKS_MAX` task.
pub fn board_status(view: &BoardView, names: &WorkspaceNames) -> Value {
    let mut out = view.to_list_json(None);
    out["workspace"] = Value::from(view.workspace_id.as_str());
    if let Some(name) = names.name_of(&view.workspace_id) {
        out["workspace_name"] = Value::from(name);
    }
    if let Some(tasks) = out["tasks"].as_array_mut() {
        let omitted = tasks.len().saturating_sub(BOARD_TASKS_MAX);
        if omitted > 0 {
            tasks.truncate(BOARD_TASKS_MAX);
            out["omitted_tasks"] = Value::from(omitted);
        }
    }
    out
}

/// Il thread per `thread_read`: titolo, stato, task collegati, riassunto e gli
/// ultimi `THREAD_READ_MESSAGES` messaggi.
pub fn thread(thread: &Thread, names: &WorkspaceNames) -> Value {
    let skip = thread.messages.len().saturating_sub(THREAD_READ_MESSAGES);
    let mut out = header(thread, names);
    out["messages"] = messages(&thread.messages[skip..], names);
    if skip > 0 {
        out["older_messages"] = Value::from(skip);
    }
    out
}

/// Titolo, canale, stato, task e proposta collegati e riassunto del thread.
pub fn header(thread: &Thread, names: &WorkspaceNames) -> Value {
    let mut out = json!({
        "title": thread.title,
        "channel": thread.channel.as_str(),
        "state": thread.state,
    });
    if let Channel::Workspace(id) = &thread.channel {
        if let Some(name) = names.name_of(id) {
            out["workspace_name"] = Value::from(name);
        }
    }
    if !thread.task_ids.is_empty() {
        out["task_ids"] = json!(thread.task_ids);
    }
    if let Some(p) = &thread.proposal_id {
        out["proposal_id"] = Value::from(p.as_str());
    }
    if let Some(s) = &thread.summary {
        out["summary"] = Value::from(s.as_str());
    }
    out
}

pub fn messages(list: &[Message], names: &WorkspaceNames) -> Value {
    Value::Array(list.iter().map(|m| message(m, names)).collect())
}

/// Un messaggio: autore, ora (UTC), testo tagliato e, per gli eventi, i dati
/// dell'evento senza i campi vuoti.
pub fn message(m: &Message, names: &WorkspaceNames) -> Value {
    let mut out = json!({
        "id": m.id,
        "from": serde_json::to_value(&m.author).unwrap_or(Value::Null),
        "at": timestamp(m.created_at),
        "body": clip_with_ellipsis(&m.body, BODY_KEPT),
    });
    if let Some(reply_to) = &m.reply_to {
        out["reply_to"] = Value::from(reply_to.as_str());
    }
    if let Some(event) = &m.event {
        out["event"] = without_nulls(serde_json::to_value(event).unwrap_or(Value::Null));
        if let Some(name) = event.workspace.as_deref().and_then(|id| names.name_of(id)) {
            out["event"]["workspace_name"] = Value::from(name);
        }
    }
    out
}

fn timestamp(ms: i64) -> Value {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| Value::from(t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)))
        .unwrap_or(Value::Null)
}

fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect::<Map<_, _>>()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::{self, msg, names};
    use crate::hub::model::{Author, HubEvent, HubEventKind, MessageKind};

    fn thread_with(n: usize) -> Thread {
        Thread { summary: Some("Login in progress.".into()), ..fixtures::thread(fixtures::numbered(n)) }
    }

    #[test]
    fn events_keep_their_data_without_empty_fields() {
        let mut m = msg("m1", Author::System, "Task completato: Login", 1_700_000_000_000);
        m.kind = MessageKind::Event;
        m.event = Some(HubEvent { status: Some("done".into()), ..HubEvent::of_task(HubEventKind::TaskDone, "t-1") });
        let v = message(&m, &names());
        assert_eq!(v["event"], json!({"kind": "task_done", "task_id": "t-1", "status": "done"}));
        assert_eq!(v["from"], json!({"type": "system"}));
        assert_eq!(v["at"], "2023-11-14T22:13:20Z");
        assert_eq!(
            message(&msg("m2", Author::Bot { name: "mario".into() }, "x", 0), &names())["from"]["name"],
            "mario"
        );
    }

    #[test]
    fn the_board_is_compact_and_truncated() {
        use crate::board::{AgentGate, Board, TaskSpec};
        let mut board = Board::new("ws-1");
        for i in 0..BOARD_TASKS_MAX + 3 {
            let spec =
                TaskSpec { id: format!("t-{i}"), title: format!("Task {i}"), task: "x".into(), ..Default::default() };
            board.create_task(spec, None, 0).unwrap();
        }
        let v = board_status(&board.view_with(None, false, &AgentGate::default()), &names());
        assert_eq!(v["workspace"], "ws-1");
        assert_eq!(v["workspace_name"], "Muse Companion");
        assert_eq!(v["tasks"].as_array().unwrap().len(), BOARD_TASKS_MAX);
        assert_eq!(v["omitted_tasks"], 3);
        assert_eq!(v["tasks"][0], json!({"id": "t-0", "title": "Task 0", "status": "queued"}));
        assert_eq!(v["counts"]["queued"], BOARD_TASKS_MAX + 3);
    }

    #[test]
    fn long_bodies_are_clipped() {
        let v = message(&msg("m", Author::User, &"é".repeat(BODY_KEPT + 50), 0), &names());
        assert_eq!(v["body"].as_str().unwrap().chars().count(), BODY_KEPT + 1);
    }

    #[test]
    fn the_thread_keeps_the_latest_messages_and_its_summary() {
        let v = thread(&thread_with(THREAD_READ_MESSAGES + 5), &names());
        let list = v["messages"].as_array().unwrap();
        assert_eq!(list.len(), THREAD_READ_MESSAGES);
        assert_eq!(list[0]["id"], "m5");
        assert_eq!((v["older_messages"].as_u64(), v["summary"].as_str()), (Some(5), Some("Login in progress.")));
        assert_eq!(v["task_ids"], json!(["t-1"]));
        let short = thread(&thread_with(2), &names());
        assert!(short.get("older_messages").is_none());
    }

    #[test]
    fn events_and_the_thread_carry_the_workspace_name_beside_the_id() {
        let mut m = msg("e1", Author::System, "Nuova proposta", 0);
        m.event = Some(HubEvent {
            workspace: Some("ws-2".into()),
            ..HubEvent::of_proposal(HubEventKind::ProposalCreated, "p-1")
        });
        let v = message(&m, &names());
        assert_eq!(
            (v["event"]["workspace"].as_str(), v["event"]["workspace_name"].as_str()),
            (Some("ws-2"), Some("Sito Web"))
        );
        let t = thread(&thread_with(1), &names());
        assert_eq!((t["channel"].as_str(), t["workspace_name"].as_str()), (Some("ws-1"), Some("Muse Companion")));
    }

    #[test]
    fn a_renamed_workspace_shows_the_new_name_and_a_removed_one_none() {
        let renamed =
            crate::hub::engine::workspace_names::WorkspaceNames::new([("ws-1".to_string(), "Nuovo".to_string())]);
        assert_eq!(thread(&thread_with(1), &renamed)["workspace_name"], "Nuovo");
        let none = crate::hub::engine::workspace_names::WorkspaceNames::default();
        assert!(thread(&thread_with(1), &none).get("workspace_name").is_none());
    }
}
