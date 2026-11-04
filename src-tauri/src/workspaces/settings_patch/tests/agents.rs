//! La parte della proposta che riguarda l'agente dei worker: `agent` e
//! `worker_agents` (modello e regole di permesso per agente).

use super::*;
use crate::agents::registry;

fn entry(model: Option<&str>, allow: &[&str]) -> AgentWorkerSettings {
    AgentWorkerSettings {
        model: model.map(str::to_string),
        allow: allow.iter().map(|s| s.to_string()).collect(),
        deny: Vec::new(),
    }
}

fn code(w: &Workspace, v: serde_json::Value) -> ErrorCode {
    patch(v).normalized(w).unwrap_err().code()
}

#[test]
fn the_agent_is_set_trimmed_and_reset_to_the_global_default() {
    let mut w = ws();
    apply(&patch(json!({"agent": format!(" {} ", agent_id())})), &mut w).unwrap();
    assert_eq!(w.agent.as_deref(), Some(agent_id()));
    apply(&patch(json!({"agent": null})), &mut w).unwrap();
    assert_eq!(w.agent, None);
    apply(&patch(json!({"agent": "  "})), &mut w).unwrap();
    assert_eq!(w.agent, None);
}

#[test]
fn an_unknown_agent_is_rejected_for_the_workspace_and_for_the_settings() {
    let w = ws();
    assert_eq!(code(&w, json!({"agent": "nope"})), ErrorCode::AgentUnknown);
    let err = patch(json!({"agent": "nope"})).normalized(&w).unwrap_err();
    assert_eq!(err.message_en(), "unknown agent: nope");
    let err = patch(json!({"worker_agents": {"nope": {"model": "m"}}})).normalized(&w).unwrap_err();
    assert_eq!(err.code(), ErrorCode::AgentUnknown);
    assert_eq!(err.message_en(), "worker_agents['nope']: unknown agent: nope");
}

#[test]
fn the_agent_is_not_a_sensitive_field() {
    assert!(patch(json!({"agent": agent_id()})).sensitive().is_empty());
}

#[test]
fn an_empty_entry_and_too_many_entries_are_rejected() {
    let w = ws();
    let err = patch(agent_patch(json!({}))).normalized(&w).unwrap_err();
    assert_eq!(err.code(), ErrorCode::RepoPatchEmpty);
    assert_eq!(err.message_en(), format!("worker_agents['{}']: has no fields", agent_id()));
    let many: serde_json::Map<String, serde_json::Value> =
        (0..=MAX_PATCH_AGENTS).map(|i| (format!("agent-{i}"), json!({"model": "m"}))).collect();
    let err = patch(json!({ "worker_agents": many })).normalized(&w).unwrap_err();
    assert_eq!(err.code(), ErrorCode::AgentsTooMany);
    assert_eq!(err.message_en(), format!("too many agents in worker_agents (max {MAX_PATCH_AGENTS})"));
}

#[test]
fn a_model_the_agent_does_not_accept_is_rejected() {
    let w = ws();
    assert_eq!(code(&w, agent_patch(json!({"model": "bad model!"}))), ErrorCode::ModelNotForAgent);
    assert!(patch(agent_patch(json!({"model": " sonnet "}))).normalized(&w).is_ok());
}

#[test]
fn only_the_fields_present_change_and_other_agents_stay() {
    let mut w = ws();
    w.worker.set_agent(agent_id(), entry(Some("sonnet"), &["Read"]));
    w.worker.set_agent("other", entry(Some("keep"), &["Keep"]));
    apply(&patch(agent_patch(json!({"deny": ["WebFetch"]}))), &mut w).unwrap();
    let settings = w.worker.for_agent(agent_id()).unwrap();
    assert_eq!(settings.model.as_deref(), Some("sonnet"));
    assert_eq!(
        (settings.allow.as_slice(), settings.deny.as_slice()),
        (&["Read".to_string()][..], &["WebFetch".to_string()][..])
    );
    assert_eq!(w.worker.for_agent("other"), Some(&entry(Some("keep"), &["Keep"])));
}

#[test]
fn an_entry_that_becomes_empty_is_removed() {
    let mut w = ws();
    w.worker.set_agent(agent_id(), entry(Some("sonnet"), &["Read"]));
    apply(&patch(agent_patch(json!({"model": null, "allow": []}))), &mut w).unwrap();
    assert!(w.worker.agents.is_empty());
    // Su un agente senza voce, azzerare non ne crea una.
    apply(&patch(agent_patch(json!({"model": null}))), &mut w).unwrap();
    assert!(w.worker.agents.is_empty());
}

#[test]
fn a_failure_in_one_agent_applies_nothing() {
    let mut w = ws();
    let before = w.clone();
    let p = patch(json!({
        "worker_rules": "x",
        "worker_agents": {agent_id(): {"model": "sonnet"}, "nope": {"model": "m"}}
    }));
    assert!(apply(&p, &mut w).is_err());
    assert_eq!(w, before);
}

#[test]
fn current_shows_the_values_of_the_touched_fields_of_each_agent() {
    let mut w = ws();
    w.worker.set_agent(agent_id(), entry(Some("sonnet"), &["Read"]));
    let cur = patch(agent_patch(json!({"model": "x", "deny": ["y"]}))).current(&w);
    let shown = &cur.worker_agents.unwrap()[agent_id()];
    assert_eq!(shown.model, Some(Some("sonnet".to_string())));
    assert_eq!(shown.deny, Some(Vec::new()));
    assert_eq!(shown.allow, None);
    // Un agente senza voce mostra i valori di fabbrica.
    let cur = patch(json!({"agent": agent_id(), "worker_agents": {"other": {"model": "m"}}})).current(&w);
    assert_eq!(cur.agent, Some(None));
    assert_eq!(cur.worker_agents.unwrap()["other"].model, Some(None));
}

#[test]
fn the_default_agent_is_the_one_of_the_registry() {
    assert_eq!(agent_id(), registry::default_agent().id());
}
