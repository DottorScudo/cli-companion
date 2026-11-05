//! Argomenti e perimetro di `set_thread_state`.

use super::*;
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

fn parsed(arguments: &str) -> Result<Invocation, ToolRejection> {
    let call = ToolCall { id: "c".into(), name: "set_thread_state".into(), arguments: arguments.into() };
    parse(&call, &secretary(&SecretaryTool::ALL))
}

fn state(state: ThreadState) -> Invocation {
    Invocation::SetThreadState { state }
}

#[test]
fn set_thread_state_takes_closed_or_open() {
    assert_eq!(parsed(r#"{"state":"closed"}"#), Ok(state(ThreadState::Closed)));
    assert_eq!(parsed(r#"{"state":"open"}"#), Ok(state(ThreadState::Open)));
}

#[test]
fn set_thread_state_rejects_other_states_ids_and_missing_arguments() {
    for arguments in [
        "{}",
        "",
        r#"{"state":"archived"}"#,
        r#"{"state":"Closed"}"#,
        r#"{"state":null}"#,
        r#"{"state":1}"#,
        r#"{"state":"closed","thread_id":"th-2"}"#,
        r#"["closed"]"#,
        "closed",
    ] {
        assert!(
            matches!(
                parsed(arguments),
                Err(ToolRejection::InvalidArguments { tool: SecretaryTool::SetThreadState, .. })
            ),
            "{arguments}"
        );
    }
}

#[test]
fn set_thread_state_exists_only_for_a_secretary_who_has_it() {
    let without = secretary(&[SecretaryTool::PostMessage, SecretaryTool::TaskAction]);
    let call = ToolCall { id: "c".into(), name: "set_thread_state".into(), arguments: r#"{"state":"closed"}"#.into() };
    assert_eq!(parse(&call, &without), Err(ToolRejection::NotAllowed("set_thread_state".into())));
    assert!(specs(&without).iter().all(|t| t.name != "set_thread_state"));
    let with = secretary(&[SecretaryTool::SetThreadState]);
    assert_eq!(parse(&call, &with), Ok(state(ThreadState::Closed)));
    let spec = &specs(&with)[0];
    assert_eq!(spec.parameters["additionalProperties"], false);
    assert_eq!(spec.parameters["properties"]["state"]["enum"], json!(["closed", "open"]));
    assert_eq!(spec.parameters["required"], json!(["state"]));
    assert!(spec.parameters["properties"].get("thread_id").is_none(), "only the current thread");
}
