//! L'agente di un task: scelto alla creazione (task del tool MCP, della UI o
//! di una proposta) e verificato quando cambia. Il registro dei test ha un
//! solo agente: le scelte fra piu' agenti sono provate in `board::agent`.

use super::*;
use crate::agents::registry;

const GONE: &str = "gone";

fn default_id() -> &'static str {
    registry::default_agent().id()
}

/// Un host con la board in pausa (i task restano in coda) e l'agente del workspace dato.
fn paused(workspace_agent: Option<&str>) -> (AutomationState, FakeHost) {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    set_workspace_agent(&h, workspace_agent);
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    (st, h)
}

fn set_workspace_agent(h: &FakeHost, agent: Option<&str>) {
    let mut w = h.ws_for("ws-1").unwrap();
    w.agent = agent.map(str::to_string);
    h.store_ws1(w);
}

fn agent_of(st: &AutomationState, h: &FakeHost, id: &str) -> String {
    view(st, h).tasks.iter().find(|t| t.task.id == id).unwrap().task.agent.clone()
}

fn with_agent(id: &str, agent: &str) -> Value {
    let mut s = spec(id, &[]);
    s["agent"] = json!(agent);
    s
}

#[test]
fn propose_tasks_rejects_an_unknown_agent_with_a_clear_error() {
    let (st, h) = paused(None);
    let err = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [with_agent("a", "nope")]})).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: nope");
    assert!(view(&st, &h).proposals.is_empty());
    // Anche l'agente del workspace, se non si puo' usare, blocca le proposte senza agente.
    set_workspace_agent(&h, Some(GONE));
    let err = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [spec("b", &[])]})).unwrap_err();
    assert_eq!(err, "task 'b': unknown agent: gone");
    // Un agente esplicito non guarda il workspace.
    call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [with_agent("c", default_id())]})).unwrap();
}

#[test]
fn an_accepted_proposal_saves_the_agent_resolved_at_that_moment() {
    let (st, h) = paused(None);
    let r = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [spec("a", &[]), with_agent("b", default_id())]}))
        .unwrap();
    let proposal = r["proposal"].as_str().unwrap();
    let written: Vec<_> = view(&st, &h).proposals[0].tasks.iter().map(|t| t.agent.clone()).collect();
    assert_eq!(written, [None, Some(default_id().to_string())]);

    // L'agente del workspace e' cambiato dopo la proposta: conta quello di adesso.
    set_workspace_agent(&h, Some(GONE));
    let err = boards::proposal_decide(&st, &h, "ws-1", proposal, true).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: gone");
    assert_eq!(view(&st, &h).proposals.len(), 1);
    set_workspace_agent(&h, Some(default_id()));
    boards::proposal_decide(&st, &h, "ws-1", proposal, true).unwrap();
    assert_eq!((agent_of(&st, &h, "a"), agent_of(&st, &h, "b")), (default_id().into(), default_id().into()));
}

#[test]
fn task_create_saves_the_agent_and_rejects_an_unusable_one() {
    let (st, h) = paused(None);
    boards::task_create(&st, &h, "ws-1", &json!({"id": "a", "title": "T", "task": "x", "agent": default_id()}))
        .unwrap();
    assert_eq!(agent_of(&st, &h, "a"), default_id());
    let err = boards::task_create(&st, &h, "ws-1", &json!({"id": "b", "title": "T", "task": "x", "agent": "nope"}))
        .unwrap_err();
    assert_eq!(err, "task 'b': unknown agent: nope");
    assert!(boards::task_create(&st, &h, "ws-1", &json!({"id": "c", "title": "T", "task": "x", "agent": "Bad Id"}))
        .unwrap_err()
        .contains("invalid agent id"));
    // Senza agente: quello del workspace, e se non e' utilizzabile l'errore lo dice.
    set_workspace_agent(&h, Some(GONE));
    let err = boards::task_create(&st, &h, "ws-1", &json!({"id": "d", "title": "T", "task": "x"})).unwrap_err();
    assert_eq!(err, "task 'd': unknown agent: gone");
    assert_eq!(view(&st, &h).tasks.len(), 1);
}

#[test]
fn update_task_checks_the_agent_only_when_the_patch_touches_it() {
    let (st, h) = paused(None);
    queue(&st, &h, vec![spec("a", &[])]);
    set_workspace_agent(&h, Some(GONE));

    // Il titolo si cambia anche se l'agente del workspace non si puo' piu' usare.
    call(&st, &h, INTAKE, "update_task", json!({"id": "a", "title": "Renamed"})).unwrap();
    assert_eq!(task(&st, &h, "a")["title"], "Renamed");
    assert_eq!(agent_of(&st, &h, "a"), default_id());

    let err = call(&st, &h, INTAKE, "update_task", json!({"id": "a", "agent": "nope"})).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: nope");
    // `null` = l'agente del workspace, risolto adesso.
    let err = call(&st, &h, INTAKE, "update_task", json!({"id": "a", "agent": null})).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: gone");
    set_workspace_agent(&h, None);
    call(&st, &h, INTAKE, "update_task", json!({"id": "a", "agent": null})).unwrap();
    assert_eq!(agent_of(&st, &h, "a"), default_id());
    assert!(call(&st, &h, INTAKE, "update_task", json!({"id": "a", "agent": ""})).is_ok());
}

#[test]
fn the_ui_update_command_goes_through_the_same_check() {
    let (st, h) = paused(None);
    queue(&st, &h, vec![spec("a", &[])]);
    let err = boards::task_update(&st, &h, "ws-1", "a", &json!({"agent": "nope"})).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: nope");
    boards::task_update(&st, &h, "ws-1", "a", &json!({"agent": default_id(), "model": "opus"})).unwrap();
    assert_eq!(task(&st, &h, "a")["model"], "opus");
}

#[test]
fn a_reviewer_proposal_goes_through_the_same_check() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    let (tab, ws) = (session_of(&st, &h, "a"), h.ws_for("ws-1").unwrap());
    let params = json!({"tasks": [with_agent("fix", "nope")]});
    let err = boards::propose_tasks(&st, &h, &ws, &tab, Some("a".into()), &params).unwrap_err();
    assert_eq!(err, "task 'fix': unknown agent: nope");
}
