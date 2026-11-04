use super::*;
use crate::agents::registry;
use crate::agents::testing::{FakeIntegration, FAKE_MODEL_SMALL};
use crate::agents::{resolve_in, IntegrationsState};

const SECOND: &str = "fake-second";
/// Valido per l'agente predefinito (forma sicura), non per quello finto.
const FOREIGN_MODEL: &str = "other-model";

fn first_id() -> &'static str {
    registry::all()[0].id()
}

fn spec(agent: Option<&str>, model: Option<&str>) -> TaskSpec {
    TaskSpec {
        id: "t1".into(),
        agent: agent.map(str::to_string),
        model: model.map(str::to_string),
        ..TaskSpec::default()
    }
}

/// Risoluzione con l'agente predefinito piu' un secondo agente finto `second`;
/// `enabled`: quali dei due sono abilitati, `default_agent` il predefinito globale.
fn resolve(
    second: &FakeIntegration,
    enabled: &[&str],
    default_agent: &str,
    spec: &TaskSpec,
    workspace_agent: Option<&str>,
) -> Result<String, String> {
    let registered = [registry::all()[0], second as &dyn AgentIntegration];
    let state = IntegrationsState {
        enabled: enabled.iter().map(|s| s.to_string()).collect(),
        default_agent: default_agent.into(),
    };
    resolve_spec_agent_with(
        &|explicit, workspace, purpose| resolve_in(&registered, &state, explicit, workspace, purpose),
        spec,
        workspace_agent,
    )
}

fn resolve_both(spec: &TaskSpec, workspace_agent: Option<&str>) -> Result<String, String> {
    resolve(&FakeIntegration::full(SECOND), &[first_id(), SECOND], first_id(), spec, workspace_agent)
}

#[test]
fn the_agent_of_the_spec_wins_then_the_workspace_then_the_global_default() {
    let first = first_id().to_string();
    let second = SECOND.to_string();
    assert_eq!(resolve_both(&spec(None, None), None), Ok(first.clone()));
    assert_eq!(resolve_both(&spec(None, None), Some(SECOND)), Ok(second.clone()));
    assert_eq!(resolve_both(&spec(Some(first_id()), None), Some(SECOND)), Ok(first));
    assert_eq!(resolve_both(&spec(Some(SECOND), None), Some(first_id())), Ok(second.clone()));
    let fake_default = FakeIntegration::full(SECOND);
    let both = [first_id(), SECOND];
    assert_eq!(resolve(&fake_default, &both, SECOND, &spec(None, None), None), Ok(second));
}

#[test]
fn an_unavailable_agent_is_an_error_prefixed_with_the_task() {
    let unknown = resolve_both(&spec(Some("nope"), None), None);
    assert_eq!(unknown, Err("task 't1': unknown agent: nope".to_string()));
    let from_workspace = resolve_both(&spec(None, None), Some("gone"));
    assert_eq!(from_workspace, Err("task 't1': unknown agent: gone".to_string()));

    let second = FakeIntegration::full(SECOND);
    let disabled = resolve(&second, &[first_id()], first_id(), &spec(Some(SECOND), None), None);
    assert_eq!(disabled, Err("task 't1': Fake is disabled: enable it in the integrations settings".to_string()));

    let bare = FakeIntegration::bare(SECOND, "fake-cli");
    let both = [first_id(), SECOND];
    let unsupported = resolve(&bare, &both, first_id(), &spec(Some(SECOND), None), None);
    assert_eq!(unsupported, Err("task 't1': Fake does not support automated sessions".to_string()));
}

#[test]
fn a_model_must_belong_to_the_agent_that_runs_the_task() {
    assert_eq!(resolve_both(&spec(Some(SECOND), Some(FAKE_MODEL_SMALL)), None), Ok(SECOND.to_string()));
    assert_eq!(resolve_both(&spec(Some(first_id()), Some(FOREIGN_MODEL)), None), Ok(first_id().to_string()));

    let wrong = resolve_both(&spec(Some(SECOND), Some(FOREIGN_MODEL)), None);
    let expected = format!("task 't1': invalid model for Fake: {FOREIGN_MODEL} (see get_workspace_settings.agents)");
    assert_eq!(wrong, Err(expected));
    // Con l'agente del workspace: stesso controllo.
    assert!(resolve_both(&spec(None, Some(FOREIGN_MODEL)), Some(SECOND)).is_err());
    assert!(resolve_both(&spec(None, Some(FAKE_MODEL_SMALL)), Some(first_id())).is_ok());
}

#[test]
fn the_registered_agents_resolve_through_the_current_state() {
    let default_id = registry::default_agent().id();
    assert_eq!(resolve_spec_agent(&spec(None, None), None), Ok(default_id.to_string()));
    assert_eq!(resolve_spec_agent(&spec(Some(default_id), None), Some("gone")), Ok(default_id.to_string()));
    assert_eq!(resolve_spec_agent(&spec(None, None), Some("gone")), Err("task 't1': unknown agent: gone".to_string()));
}
