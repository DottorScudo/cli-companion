use super::*;
use crate::hub::settings::SecretaryChannels;

fn secretary(tools: &[SecretaryTool]) -> Secretary {
    Secretary {
        name: "mario".into(),
        image: None,
        system_prompt: "Report.".into(),
        tools: tools.to_vec(),
        channels: SecretaryChannels::All,
        preset: None,
    }
}

fn call(name: &str, arguments: &str) -> ToolCall {
    ToolCall { id: "c".into(), name: name.into(), arguments: arguments.into() }
}

#[test]
fn only_the_allowed_tools_are_offered() {
    let s = secretary(&[SecretaryTool::PostMessage, SecretaryTool::BoardStatus]);
    let names: Vec<&str> = specs(&s).iter().map(|t| t.name).collect();
    assert_eq!(names, ["board_status", "post_message"]);
    assert!(specs(&secretary(&[])).is_empty());
}

#[test]
fn every_schema_is_a_closed_object_with_an_english_description() {
    for tool in SecretaryTool::ALL {
        let s = spec(tool);
        assert_eq!(s.name, tool.code());
        assert_eq!(s.parameters["type"], "object");
        assert_eq!(s.parameters["additionalProperties"], false);
        assert!(s.description.is_ascii() && !s.description.is_empty());
    }
}

#[test]
fn schema_properties_match_the_argument_structs() {
    let all = secretary(&SecretaryTool::ALL);
    let samples = [
        ("board_status", json!({"workspace": "ws-1"})),
        ("thread_read", json!({})),
        ("post_message", json!({"body": "x"})),
        ("open_plan_task", json!({"goal": "Add a dark theme"})),
        ("decide_proposal", json!({"accept": true})),
        ("relay_answer", json!({"body": "Use the second option."})),
    ];
    for (name, sample) in samples {
        let tool = SecretaryTool::ALL.into_iter().find(|t| t.code() == name).unwrap();
        let props = spec(tool).parameters["properties"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        let keys = sample.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(props, keys, "{name}");
        assert!(parse(&call(name, &sample.to_string()), &all).is_ok(), "{name}");
    }
}

#[test]
fn a_tool_outside_the_list_is_not_allowed() {
    let s = secretary(&[SecretaryTool::ThreadRead]);
    assert_eq!(parse(&call("board_status", "{}"), &s), Err(ToolRejection::NotAllowed("board_status".into())));
    assert_eq!(parse(&call("rm\n-rf", "{}"), &s), Err(ToolRejection::NotAllowed("rm -rf".into())));
    let long = "x".repeat(200);
    let Err(ToolRejection::NotAllowed(name)) = parse(&call(&long, "{}"), &s) else { panic!() };
    assert!(name.chars().count() <= UNKNOWN_NAME_KEPT + 1);
}

#[test]
fn arguments_are_validated_against_the_schema() {
    let s = secretary(&SecretaryTool::ALL);
    assert_eq!(parse(&call("thread_read", ""), &s), Ok(Invocation::ThreadRead));
    assert_eq!(parse(&call("board_status", "{}"), &s), Ok(Invocation::BoardStatus { workspace: None }));
    assert_eq!(
        parse(&call("board_status", r#"{"workspace":" ws-2 "}"#), &s),
        Ok(Invocation::BoardStatus { workspace: Some("ws-2".into()) })
    );
    assert_eq!(
        parse(&call("board_status", r#"{"workspace":" Sito Web "}"#), &s),
        Ok(Invocation::BoardStatus { workspace: Some("Sito Web".into()) })
    );
    assert_eq!(
        parse(&call("post_message", r#"{"body":"  Controllo.  "}"#), &s),
        Ok(Invocation::PostMessage { body: "Controllo.".into() })
    );
    let too_long = format!(r#"{{"workspace":"{}"}}"#, "x".repeat(REFERENCE_MAX + 1));
    for (name, args) in [
        ("thread_read", r#"{"limit":3}"#),
        ("thread_read", "[]"),
        ("thread_read", "{"),
        ("board_status", too_long.as_str()),
        ("board_status", r#"{"workspace":3}"#),
        ("post_message", "{}"),
        ("post_message", r#"{"body":"   "}"#),
        ("post_message", r#"{"body":"a","extra":1}"#),
    ] {
        let err = parse(&call(name, args), &s).unwrap_err();
        assert!(matches!(err, ToolRejection::InvalidArguments { .. }), "{name} {args}");
        assert!(err.for_model().contains(name));
    }
    let long = json!({ "body": "a".repeat(BODY_MAX + 1) }).to_string();
    assert!(parse(&call("post_message", &long), &s).is_err());
}

#[test]
fn the_planning_tools_validate_their_arguments() {
    let s = secretary(&SecretaryTool::ALL);
    assert_eq!(
        parse(&call("open_plan_task", r#"{"goal":"  Add X to the app\u0007 "}"#), &s),
        Ok(Invocation::OpenPlanTask { goal: "Add X to the app".into() })
    );
    assert_eq!(
        parse(&call("decide_proposal", r#"{"accept":false}"#), &s),
        Ok(Invocation::DecideProposal { accept: false })
    );
    for (name, args) in [
        ("open_plan_task", "{}"),
        ("open_plan_task", r#"{"goal":"   "}"#),
        ("open_plan_task", r#"{"goal":"x","workspace":"ws-1"}"#),
        ("decide_proposal", "{}"),
        ("decide_proposal", r#"{"accept":"yes"}"#),
        ("decide_proposal", r#"{"accept":true,"proposal_id":"p-1"}"#),
    ] {
        let err = parse(&call(name, args), &s).unwrap_err();
        assert!(matches!(err, ToolRejection::InvalidArguments { .. }), "{name} {args}");
    }
    let long = json!({ "goal": "é".repeat(GOAL_MAX + 1) }).to_string();
    assert!(parse(&call("open_plan_task", &long), &s).is_err());
    assert!(parse(&call("open_plan_task", &json!({ "goal": "é".repeat(GOAL_MAX) }).to_string()), &s).is_ok());
    // Un segretario senza i tool nuovi (salvato prima) non li ha.
    let old = secretary(&[SecretaryTool::BoardStatus]);
    assert_eq!(
        parse(&call("decide_proposal", r#"{"accept":true}"#), &old),
        Err(ToolRejection::NotAllowed("decide_proposal".into()))
    );
}

#[test]
fn relay_answer_takes_only_a_body_within_the_answer_limit() {
    let s = secretary(&SecretaryTool::ALL);
    assert_eq!(
        parse(&call("relay_answer", r#"{"body":"  Usa Postgres.  "}"#), &s),
        Ok(Invocation::RelayAnswer { body: "Usa Postgres.".into() })
    );
    let at_limit = json!({ "body": "é".repeat(ANSWER_MAX) }).to_string();
    assert!(parse(&call("relay_answer", &at_limit), &s).is_ok());
    let too_long = json!({ "body": "é".repeat(ANSWER_MAX + 1) }).to_string();
    for args in ["{}", r#"{"body":"   "}"#, r#"{"body":"x","question_id":"q-1-1"}"#, too_long.as_str()] {
        let err = parse(&call("relay_answer", args), &s).unwrap_err();
        assert!(matches!(err, ToolRejection::InvalidArguments { tool: SecretaryTool::RelayAnswer, .. }), "{args}");
    }
    let old = secretary(&[SecretaryTool::DecideProposal]);
    assert_eq!(
        parse(&call("relay_answer", r#"{"body":"x"}"#), &old),
        Err(ToolRejection::NotAllowed("relay_answer".into()))
    );
}

#[test]
fn the_board_is_the_channel_one_or_one_of_the_secretary_channels() {
    let ws = |id: &str| Channel::Workspace(id.into());
    let mut s = secretary(&[SecretaryTool::BoardStatus]);
    assert_eq!(board_workspace(None, &ws("ws-1"), &s), Ok("ws-1".into()));
    assert_eq!(board_workspace(None, &Channel::General, &s), Err(BoardAccessError::NoChannelBoard));
    assert_eq!(board_workspace(Some("ws-2".into()), &Channel::General, &s), Ok("ws-2".into()));
    s.channels = SecretaryChannels::Only { channels: vec![Channel::General, ws("ws-1")] };
    assert_eq!(board_workspace(Some("ws-1".into()), &Channel::General, &s), Ok("ws-1".into()));
    assert_eq!(board_workspace(Some("ws-2".into()), &ws("ws-1"), &s), Err(BoardAccessError::NotAllowed("ws-2".into())));
}
