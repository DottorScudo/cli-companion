//! Il modello con cui parte un worker: quello del task, poi quello del
//! workspace per l'agente del task.

use super::*;
use crate::agents::registry;
use crate::workspaces::AgentWorkerSettings;

const WORKSPACE_MODEL: &str = "opus";
const TASK_MODEL: &str = "sonnet";

/// Un task creato dall'interfaccia senza modello (le proposte dei tool ne vogliono uno).
fn create_without_model(st: &AutomationState, h: &FakeHost) {
    boards::task_create(st, h, "ws-1", &json!({"title": "Task a", "task": "do a"})).unwrap();
}

fn set_workspace_model(h: &FakeHost, agent: &str) {
    let settings = AgentWorkerSettings { model: Some(WORKSPACE_MODEL.into()), ..Default::default() };
    h.worker_agents.lock().unwrap().insert(agent.to_string(), settings);
}

fn launched_with(h: &FakeHost) -> String {
    let command = h.spawns().remove(0).command;
    command.rsplit("model=").next().unwrap().to_string()
}

#[test]
fn a_task_without_a_model_gets_the_workspace_model_of_its_agent() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    set_workspace_model(&h, registry::default_agent().id());
    create_without_model(&st, &h);
    assert_eq!(launched_with(&h), WORKSPACE_MODEL);
}

#[test]
fn the_model_of_the_task_wins_over_the_workspace_model() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    set_workspace_model(&h, registry::default_agent().id());
    queue(&st, &h, vec![spec("a", &[])]);
    assert_eq!(launched_with(&h), TASK_MODEL);
}

#[test]
fn the_model_of_another_agent_is_not_used() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    set_workspace_model(&h, "some-other-agent");
    create_without_model(&st, &h);
    assert_eq!(launched_with(&h), "-", "no model: the agent decides");
}
