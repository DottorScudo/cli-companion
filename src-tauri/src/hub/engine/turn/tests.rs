use super::*;
use crate::hub::engine::fake::{call, calls, text, FakeModel};
use crate::hub::engine::fixtures::secretary;
use crate::hub::settings::SecretaryTool;
use std::sync::Mutex;
use tauri::async_runtime::block_on;

/// Host finto: board e thread fissi, registra messaggi e avvisi.
#[derive(Default)]
struct FakeHost {
    posted: Mutex<Vec<String>>,
    notices: Mutex<Vec<String>>,
    boards: Mutex<Vec<Option<String>>>,
}

impl TurnHost for FakeHost {
    async fn board_status(&self, workspace: Option<String>) -> Result<Value, String> {
        self.boards.lock().unwrap().push(workspace.clone());
        match workspace.as_deref() {
            Some("gone") => Err("Workspace `gone` not found.".to_string()),
            _ => Ok(json!({ "tasks": [{ "id": "t-1", "status": "running" }] })),
        }
    }

    async fn thread_read(&self) -> Result<Value, String> {
        Ok(json!({ "title": "Login", "big": "x".repeat(TOOL_RESULT_MAX * 2) }))
    }

    async fn post_message(&self, body: String) -> Result<(), String> {
        self.posted.lock().unwrap().push(body);
        Ok(())
    }

    async fn notice(&self, text: String) {
        self.notices.lock().unwrap().push(text);
    }
}

fn spec(s: &Secretary, max_steps: u32) -> TurnSpec<'_> {
    TurnSpec { secretary: s, system: "system".into(), max_steps }
}

fn run(model: &FakeModel, host: &FakeHost, s: &Secretary, max_steps: u32) -> (TurnEnd, Vec<ChatMessage>) {
    let mut conversation = vec![ChatMessage::User("@mario a che punto siamo?".into())];
    let end = block_on(run_turn(model, host, &spec(s, max_steps), &mut conversation));
    (end, conversation)
}

#[test]
fn a_text_reply_ends_the_turn() {
    let model = FakeModel::new(vec![text("Il task t-1 è in corso.")]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Il task t-1 è in corso.".into()));
    assert_eq!(conversation.len(), 2);
    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].system, "system");
    assert_eq!(requests[0].tools, ["board_status", "thread_read", "post_message"]);
}

#[test]
fn tools_run_and_their_results_go_back_to_the_model() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "board_status", "{}"), call("c2", "post_message", r#"{"body":"Controllo la board."}"#)]),
        calls(vec![call("c3", "thread_read", "")]),
        text("t-1 è in corso."),
    ]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("t-1 è in corso.".into()));
    assert_eq!(*host.posted.lock().unwrap(), ["Controllo la board."]);
    assert_eq!(*host.boards.lock().unwrap(), [None]);
    assert!(host.notices.lock().unwrap().is_empty());
    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    let second = &requests[1].messages;
    assert!(matches!(&second[1], ChatMessage::Assistant { tool_calls, .. } if tool_calls.len() == 2));
    assert!(matches!(&second[2], ChatMessage::Tool { call_id, content } if call_id == "c1" && content.contains("t-1")));
    assert!(matches!(&second[3], ChatMessage::Tool { content, .. } if content == r#"{"posted":true}"#));
    // L'esito lungo di thread_read e' tagliato.
    let ChatMessage::Tool { content, .. } = &conversation[5] else { panic!("{conversation:?}") };
    assert_eq!(content.chars().count(), TOOL_RESULT_MAX + 1);
}

#[test]
fn a_tool_outside_the_list_is_refused_with_a_notice_and_the_turn_goes_on() {
    let mut s = secretary("mario");
    s.tools = vec![SecretaryTool::ThreadRead];
    let model =
        FakeModel::new(vec![calls(vec![call("c1", "board_status", "{}")]), text("Non posso leggere la board.")]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &s, 8);
    assert_eq!(end, TurnEnd::Reply("Non posso leggere la board.".into()));
    assert!(host.boards.lock().unwrap().is_empty());
    let notices = host.notices.lock().unwrap();
    assert_eq!(notices.len(), 1);
    assert!(notices[0].contains("«board_status»"), "{notices:?}");
    let back = &model.requests()[1].messages[2];
    assert!(matches!(back, ChatMessage::Tool { content, .. } if content.contains("not available to you")));
    assert_eq!(model.requests()[0].tools, ["thread_read"]);
}

#[test]
fn invalid_arguments_and_tool_errors_return_an_error_to_the_model() {
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "post_message", r#"{"text":"x"}"#),
            call("c2", "board_status", r#"{"workspace":"gone"}"#),
        ]),
        text("Scusa."),
    ]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Scusa.".into()));
    assert!(host.posted.lock().unwrap().is_empty());
    assert_eq!(host.notices.lock().unwrap().len(), 1, "solo il tool rifiutato e' un avviso");
    let messages = &model.requests()[1].messages;
    assert!(
        matches!(&messages[2], ChatMessage::Tool { content, .. } if content.contains("Invalid arguments for `post_message`"))
    );
    assert!(matches!(&messages[3], ChatMessage::Tool { content, .. } if content.contains("not found")));
}

#[test]
fn the_step_limit_stops_the_turn_without_running_the_last_calls() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "thread_read", "")]),
        calls(vec![call("c2", "post_message", r#"{"body":"ancora"}"#)]),
    ]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 2);
    assert_eq!(end, TurnEnd::StepLimit);
    assert_eq!(model.requests().len(), 2);
    assert!(host.posted.lock().unwrap().is_empty());
    // L'ultima conversazione valida: utente, chiamata, esito.
    assert_eq!(conversation.len(), 3);
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
}

#[test]
fn endpoint_errors_end_the_turn_with_the_error() {
    for error in [ModelError::Unreachable, ModelError::Timeout, ModelError::Http(500), ModelError::Empty] {
        let model = FakeModel::new(vec![Err(error)]);
        let (end, conversation) = run(&model, &FakeHost::default(), &secretary("mario"), 8);
        assert_eq!(end, TurnEnd::Failed(error));
        assert_eq!(conversation.len(), 1);
    }
    let model = FakeModel::new(vec![calls(vec![call("c1", "thread_read", "")]), Err(ModelError::Timeout)]);
    let (end, conversation) = run(&model, &FakeHost::default(), &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Failed(ModelError::Timeout));
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
}

#[test]
fn compaction_shortens_old_tool_results_and_keeps_a_valid_start() {
    let mut conversation = Vec::new();
    for i in 0..CONVERSATION_MAX {
        conversation.push(ChatMessage::User(format!("u{i}")));
        conversation.push(ChatMessage::Assistant { text: None, tool_calls: vec![call("c", "thread_read", "")] });
        conversation.push(ChatMessage::Tool { call_id: "c".into(), content: "x".repeat(TOOL_RESULT_KEPT * 3) });
    }
    compact(&mut conversation);
    assert!(conversation.len() <= CONVERSATION_MAX);
    assert!(matches!(&conversation[0], ChatMessage::User(_)));
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
    for m in &conversation {
        if let ChatMessage::Tool { content, .. } = m {
            assert_eq!(content.chars().count(), TOOL_RESULT_KEPT + 1);
        }
    }
    let mut short = vec![ChatMessage::User("x".into())];
    compact(&mut short);
    assert_eq!(short.len(), 1);
}

#[test]
fn the_summary_comes_from_the_model_or_is_kept() {
    let conversation = [ChatMessage::User("stato?".into())];
    let model = FakeModel::new(vec![text("  t-1 running.\n\nUser waits for the login.  ")]);
    let summary = block_on(updated_summary(&model, Some("Old."), &conversation));
    assert_eq!(summary.as_deref(), Some("t-1 running.\nUser waits for the login."));
    let request = &model.requests()[0];
    assert!(request.tools.is_empty());
    assert!(matches!(&request.messages[0], ChatMessage::User(t) if t.contains("Old.") && t.contains("stato?")));
    for reply in [Err(ModelError::Unreachable), calls(vec![call("c", "thread_read", "")]), text("   ")] {
        assert_eq!(block_on(updated_summary(&FakeModel::new(vec![reply]), Some("Old."), &conversation)), None);
    }
}
