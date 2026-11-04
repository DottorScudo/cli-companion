use super::*;
use crate::board::Board;

/// Una board scritta prima delle impostazioni per agente, con proposte che
/// toccano `worker_model`, `worker_allow` e `worker_deny`.
const V1_SETTINGS: &str = include_str!("../fixtures/board_v1_settings.json");

const AGENT: &str = "agent-a";

/// La board della fixture, senza la busta: e' quello che vede il passo di migrazione.
fn fixture() -> Value {
    let mut envelope: Value = serde_json::from_str(V1_SETTINGS).unwrap();
    envelope["data"].take()
}

fn patches(board: &Value) -> Vec<&Value> {
    board["settings_proposals"].as_array().unwrap().iter().map(|p| &p["patch"]).collect()
}

fn migrated() -> Value {
    move_patch_fields(fixture(), AGENT)
}

#[test]
fn legacy_fields_of_a_proposal_move_to_the_agent_entry() {
    let out = migrated();
    let patches = patches(&out);
    assert_eq!(
        *patches[0],
        json!({"worker_rules": "new", "worker_agents": {AGENT: {"model": "big", "allow": ["Read"], "deny": ["Edit"]}}})
    );
    // `null` e liste vuote hanno un significato in una patch (azzerano): restano.
    assert_eq!(*patches[1], json!({"worker_agents": {AGENT: {"model": null, "allow": []}}}));
}

#[test]
fn proposals_without_legacy_fields_and_other_data_are_untouched() {
    let original = fixture();
    let out = migrated();
    assert_eq!(*patches(&out)[2], *patches(&original)[2]);
    assert_eq!(out["tasks"], original["tasks"]);
    assert_eq!(out["settings_proposals"][0]["reason"], original["settings_proposals"][0]["reason"]);
}

#[test]
fn an_existing_agent_entry_is_not_overwritten_and_the_step_is_idempotent() {
    let board = json!({"settings_proposals": [
        {"patch": {"worker_model": "old", "worker_agents": {AGENT: {"model": "new"}}}}
    ]});
    let out = move_patch_fields(board, AGENT);
    assert_eq!(out["settings_proposals"][0]["patch"], json!({"worker_agents": {AGENT: {"model": "new"}}}));
    assert_eq!(move_patch_fields(out.clone(), AGENT), out);
}

#[test]
fn a_board_without_proposals_or_with_odd_entries_passes_through() {
    for odd in [
        json!({"tasks": []}),
        json!({"settings_proposals": 3}),
        json!({"settings_proposals": [1, {"patch": 4}, {"id": "x"}]}),
        json!({"settings_proposals": [{"patch": {"worker_model": "m", "worker_agents": 3}}]}),
    ] {
        let out = move_patch_fields(odd.clone(), AGENT);
        assert_eq!(out.get("tasks"), odd.get("tasks"));
    }
    let malformed = json!({"settings_proposals": [{"patch": {"worker_model": "m", "worker_agents": 3}}]});
    assert_eq!(patches(&move_patch_fields(malformed, AGENT))[0], &json!({"worker_agents": 3}));
}

#[test]
fn the_step_uses_the_default_agent_of_the_registry() {
    let out = v1_to_v2(fixture()).unwrap();
    let id = registry::default_agent().id();
    assert_eq!(patches(&out)[0]["worker_agents"][id]["model"], "big");
}

#[test]
fn an_old_board_with_such_proposals_loads_as_current_settings_patches() {
    let board = Board::from_json(V1_SETTINGS, 1).unwrap();
    let id = registry::default_agent().id();
    let first = &board.settings_proposals[0].patch;
    let entry = &first.worker_agents.as_ref().unwrap()[id];
    assert_eq!(entry.model, Some(Some("big".to_string())));
    assert_eq!(
        (entry.allow.as_deref(), entry.deny.as_deref()),
        (Some(&["Read".to_string()][..]), Some(&["Edit".to_string()][..]))
    );
    let reset = &board.settings_proposals[1].patch.worker_agents.as_ref().unwrap()[id];
    assert_eq!((reset.model.clone(), reset.allow.clone(), reset.deny.clone()), (Some(None), Some(vec![]), None));
    assert_eq!(first.worker_rules.as_deref(), Some("new"));
    assert_eq!(board.settings_proposals.len(), 3);
}
