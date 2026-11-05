//! Argomenti e perimetro dei tool che cambiano la board: `task_action` e
//! `board_control`.

use super::*;
use crate::board::{MAX_PARALLEL_LIMIT, MIN_PARALLEL_LIMIT};
use crate::hub::engine::tools::task_details::TASK_ID_MAX;
use crate::hub::settings::SecretaryChannels;

fn secretary(tools: &[SecretaryTool]) -> Secretary {
    Secretary {
        name: "ugo".into(),
        image: None,
        system_prompt: "Operate.".into(),
        tools: tools.to_vec(),
        channels: SecretaryChannels::All,
        preset: None,
    }
}

fn parsed(name: &str, arguments: &str) -> Result<Invocation, ToolRejection> {
    let call = ToolCall { id: "c".into(), name: name.into(), arguments: arguments.into() };
    parse(&call, &secretary(&SecretaryTool::ALL))
}

fn reason(result: Result<Invocation, ToolRejection>) -> String {
    match result {
        Err(ToolRejection::InvalidArguments { reason, .. }) => reason,
        other => panic!("{other:?}"),
    }
}

fn action(task_id: &str, op: TaskOp, workspace: Option<&str>) -> Invocation {
    Invocation::TaskAction { workspace: workspace.map(str::to_string), task_id: task_id.into(), op }
}

#[test]
fn task_action_takes_a_task_an_action_and_an_optional_workspace() {
    assert_eq!(
        parsed("task_action", r#"{"task_id":" t-1 ","action":"retry"}"#),
        Ok(action("t-1", TaskOp::Retry, None))
    );
    assert_eq!(
        parsed("task_action", r#"{"task_id":"t-1","action":"to_top","workspace":"Sito Web","before_task_id":""}"#),
        Ok(action("t-1", TaskOp::ToTop, Some("Sito Web")))
    );
    assert_eq!(
        parsed("task_action", r#"{"task_id":"t-1","action":"move_before","before_task_id":"t-2"}"#),
        Ok(action("t-1", TaskOp::MoveBefore { before_task_id: "t-2".into() }, None))
    );
}

#[test]
fn task_action_rejects_bad_arguments_with_a_clear_reason() {
    let long = "x".repeat(TASK_ID_MAX + 1);
    assert!(reason(parsed("task_action", r#"{"task_id":"t-1","action":"delete"}"#)).contains("schema"));
    assert!(reason(parsed("task_action", r#"{"task_id":"t-1","action":"mark_done"}"#)).contains("schema"));
    assert!(reason(parsed("task_action", r#"{"task_id":"t-1"}"#)).contains("schema"));
    assert_eq!(
        reason(parsed("task_action", r#"{"task_id":"t-1","action":"move_before"}"#)),
        "`move_before` needs `before_task_id`"
    );
    assert_eq!(
        reason(parsed("task_action", r#"{"task_id":"t-1","action":"retry","before_task_id":"t-2"}"#)),
        "`before_task_id` is only for `move_before`"
    );
    assert!(reason(parsed("task_action", r#"{"task_id":" ","action":"retry"}"#)).starts_with("`task_id` must be"));
    let too_long = format!(r#"{{"task_id":"t-1","action":"move_before","before_task_id":"{long}"}}"#);
    assert!(reason(parsed("task_action", &too_long)).starts_with("`before_task_id` must be"));
}

#[test]
fn board_control_takes_pause_parallelism_or_both_within_the_board_limits() {
    let control = |paused, max_parallel| Invocation::BoardControl {
        workspace: None,
        change: BoardChange { paused, max_parallel },
    };
    assert_eq!(parsed("board_control", r#"{"paused":true}"#), Ok(control(Some(true), None)));
    let max = format!(r#"{{"max_parallel":{MAX_PARALLEL_LIMIT},"paused":false}}"#);
    assert_eq!(parsed("board_control", &max), Ok(control(Some(false), Some(MAX_PARALLEL_LIMIT))));
    let min = format!(r#"{{"max_parallel":{MIN_PARALLEL_LIMIT}}}"#);
    assert_eq!(parsed("board_control", &min), Ok(control(None, Some(MIN_PARALLEL_LIMIT))));
    for bad in [0, i64::from(MAX_PARALLEL_LIMIT) + 1, -1, 300] {
        let e = reason(parsed("board_control", &format!(r#"{{"max_parallel":{bad}}}"#)));
        assert!(e.starts_with("`max_parallel` must be an integer from 1 to 16"), "{bad}: {e}");
    }
    assert_eq!(reason(parsed("board_control", "{}")), "pass `paused`, `max_parallel` or both");
    assert_eq!(reason(parsed("board_control", r#"{"workspace":"ws-1"}"#)), "pass `paused`, `max_parallel` or both");
    assert!(reason(parsed("board_control", r#"{"max_parallel":2.5}"#)).contains("schema"));
}

#[test]
fn only_the_thread_board_can_be_changed_and_in_general_one_of_the_secretary_channels() {
    let ws = |id: &str| Channel::Workspace(id.into());
    let mut s = secretary(&[SecretaryTool::TaskAction]);
    assert_eq!(change_workspace(None, &ws("ws-1"), &s), Ok("ws-1".into()));
    assert_eq!(change_workspace(Some("ws-1".into()), &ws("ws-1"), &s), Ok("ws-1".into()));
    assert_eq!(
        change_workspace(Some("ws-2".into()), &ws("ws-1"), &s),
        Err(BoardAccessError::OutsideThread("ws-2".into()))
    );
    assert_eq!(change_workspace(None, &Channel::General, &s), Err(BoardAccessError::NoChannelBoard));
    assert_eq!(change_workspace(Some("ws-2".into()), &Channel::General, &s), Ok("ws-2".into()));
    s.channels = SecretaryChannels::Only { channels: vec![Channel::General, ws("ws-1")] };
    assert_eq!(
        change_workspace(Some("ws-2".into()), &Channel::General, &s),
        Err(BoardAccessError::NotAllowed("ws-2".into()))
    );
}

#[test]
fn the_board_tools_exist_only_for_a_secretary_who_has_them() {
    let reporter = secretary(&[SecretaryTool::BoardStatus, SecretaryTool::TaskDetails, SecretaryTool::PostMessage]);
    for (name, args) in
        [("task_action", r#"{"task_id":"t-1","action":"retry"}"#), ("board_control", r#"{"paused":true}"#)]
    {
        let call = ToolCall { id: "c".into(), name: name.into(), arguments: args.into() };
        assert_eq!(parse(&call, &reporter), Err(ToolRejection::NotAllowed(name.into())));
    }
    let names: Vec<&str> = specs(&reporter).iter().map(|t| t.name).collect();
    assert_eq!(names, ["board_status", "task_details", "post_message"]);
}
