//! L'agente dei task nelle operazioni della coda. Il registro dei test ha un
//! solo agente (il predefinito): le scelte fra piu' agenti sono provate in
//! `agent/tests.rs`, qui contano quando e dove si risolve e si salva.

use super::*;
use crate::agents::registry;
use serde_json::json;

/// Id che nessun agente registrato ha.
const GONE: &str = "gone";

fn default_id() -> &'static str {
    registry::default_agent().id()
}

fn spec(id: &str, agent: Option<&str>) -> TaskSpec {
    TaskSpec {
        id: id.into(),
        title: format!("Task {id}"),
        task: format!("do {id}"),
        agent: agent.map(str::to_string),
        ..TaskSpec::default()
    }
}

fn agent_of(b: &Board, id: &str) -> String {
    b.task(id).unwrap().agent.clone()
}

#[test]
fn a_created_task_saves_the_resolved_agent() {
    let mut b = Board::new("ws-1");
    b.create_task(spec("explicit", Some(default_id())), Some(GONE), 1).unwrap();
    b.create_task(spec("workspace", None), Some(default_id()), 2).unwrap();
    b.create_task(spec("global", None), None, 3).unwrap();
    for id in ["explicit", "workspace", "global"] {
        assert_eq!(agent_of(&b, id), default_id(), "{id}");
    }
}

#[test]
fn an_unusable_agent_rejects_the_creation_with_a_clear_error() {
    let mut b = Board::new("ws-1");
    let explicit = b.create_task(spec("a", Some(GONE)), None, 1).unwrap_err();
    assert_eq!(explicit, "task 'a': unknown agent: gone");
    let from_workspace = b.create_task(spec("b", None), Some(GONE), 1).unwrap_err();
    assert_eq!(from_workspace, "task 'b': unknown agent: gone");
    assert!(b.tasks.is_empty());
}

#[test]
fn a_model_the_agent_does_not_accept_rejects_the_creation() {
    let mut b = Board::new("ws-1");
    let bad = TaskSpec { model: Some("a b".into()), ..spec("a", None) };
    let expected = format!("task 'a': invalid model for {}: a b", registry::default_agent().display_name());
    assert!(b.create_task(bad, None, 1).unwrap_err().starts_with(&expected));
}

#[test]
fn a_proposal_keeps_the_agent_as_written_and_resolves_it_when_accepted() {
    let mut b = Board::new("ws-1");
    let specs = vec![spec("plain", None), spec("explicit", Some(default_id()))];
    b.add_proposal("p-1".into(), "tab-1", None, specs, None, 1).unwrap();
    let kept: Vec<_> = b.proposal("p-1").unwrap().tasks.iter().map(|t| t.agent.clone()).collect();
    assert_eq!(kept, [None, Some(default_id().to_string())]);

    // Vale il workspace al momento dell'accettazione: se non e' utilizzabile la proposta resta.
    let err = b.accept_proposal("p-1", Some(GONE), 2).unwrap_err();
    assert_eq!(err, "task 'plain': unknown agent: gone");
    assert_eq!(b.proposals.len(), 1);
    assert!(b.tasks.is_empty());

    b.accept_proposal("p-1", Some(default_id()), 3).unwrap();
    assert_eq!(agent_of(&b, "plain"), default_id());
    assert_eq!(agent_of(&b, "explicit"), default_id());
}

#[test]
fn a_proposal_with_an_unusable_agent_is_rejected_when_proposed() {
    let mut b = Board::new("ws-1");
    let err = b.add_proposal("p-1".into(), "tab-1", None, vec![spec("a", Some(GONE))], None, 1).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: gone");
    assert!(b.proposals.is_empty());
}

#[test]
fn the_saved_agent_does_not_follow_later_changes_of_the_defaults() {
    let mut b = Board::new("ws-1");
    b.create_task(spec("a", None), None, 1).unwrap();
    // Il workspace ora ha un agente inutilizzabile: il task non lo guarda
    // finche' non si toccano agente o modello.
    b.update_task("a", &json!({"title": "Renamed"}), Some(GONE), 2).unwrap();
    assert_eq!(b.task("a").unwrap().title, "Renamed");
    assert_eq!(agent_of(&b, "a"), default_id());
}

#[test]
fn a_task_whose_agent_disappeared_can_be_edited_but_not_re_modelled() {
    let mut b = Board::new("ws-1");
    b.create_task(spec("a", None), None, 1).unwrap();
    b.task_mut("a").unwrap().agent = GONE.into();
    b.update_task("a", &json!({"title": "Still editable"}), None, 2).unwrap();
    assert_eq!(agent_of(&b, "a"), GONE);
    let err = b.update_task("a", &json!({"model": "other-model"}), None, 3).unwrap_err();
    assert_eq!(err, "task 'a': unknown agent: gone");
    assert_eq!(b.task("a").unwrap().model, None);
}

#[test]
fn updating_the_agent_validates_it_and_null_goes_back_to_the_defaults() {
    let mut b = Board::new("ws-1");
    b.create_task(spec("a", None), None, 1).unwrap();
    b.task_mut("a").unwrap().agent = GONE.into();

    let unknown = b.update_task("a", &json!({"agent": "nope"}), None, 2).unwrap_err();
    assert_eq!(unknown, "task 'a': unknown agent: nope");
    assert_eq!(agent_of(&b, "a"), GONE);

    // `null`: l'agente del workspace, risolto adesso (qui inutilizzabile).
    let bad_workspace = b.update_task("a", &json!({"agent": null}), Some(GONE), 3).unwrap_err();
    assert_eq!(bad_workspace, "task 'a': unknown agent: gone");
    b.update_task("a", &json!({"agent": null}), None, 4).unwrap();
    assert_eq!(agent_of(&b, "a"), default_id());

    b.task_mut("a").unwrap().agent = GONE.into();
    b.update_task("a", &json!({"agent": default_id()}), Some(GONE), 5).unwrap();
    assert_eq!(agent_of(&b, "a"), default_id());
}

#[test]
fn a_model_in_an_update_is_checked_against_the_agent_of_the_task() {
    let mut b = Board::new("ws-1");
    b.create_task(spec("a", None), None, 1).unwrap();
    b.update_task("a", &json!({"model": "other-model"}), Some(GONE), 2).unwrap();
    assert_eq!(b.task("a").unwrap().model.as_deref(), Some("other-model"));
    assert_eq!(agent_of(&b, "a"), default_id());
}
