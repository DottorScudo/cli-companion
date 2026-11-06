//! `get_workspace_settings.bots`: i Bot che un modello puo' nominare nei task.

use super::*;
use crate::agents::registry;
use crate::agents::Readiness;

fn bots(st: &AutomationState, h: &FakeHost) -> Vec<Value> {
    let out = call(st, h, INTAKE, "get_workspace_settings", json!({})).unwrap();
    out["bots"].as_array().unwrap().clone()
}

fn bot<'a>(list: &'a [Value], id: &str) -> &'a Value {
    list.iter().find(|b| b["id"] == id).unwrap()
}

#[test]
fn every_bot_is_listed_in_order_with_the_documented_fields() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    h.edit_bots(|bots| *bots = crate::bots::default_agent_bots());
    let list = bots(&st, &h);
    let ids: Vec<_> = list.iter().map(|b| b["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "general",
            "intake",
            "setup",
            "skill-wizard",
            "reviewer",
            "planner",
            "quick-answer",
            "bug-fixer",
            "researcher",
            "test-writer",
            "docs-writer",
            "refactorer",
            "git-operator",
            "release"
        ]
    );

    let agent = registry::default_agent().id();
    let strong = crate::agents::tier_models(registry::default_agent()).unwrap().strong;
    assert_eq!(
        *bot(&list, "intake"),
        json!({
            "id": "intake",
            "name": "Intake",
            "task": "Turns requests into queue tasks.",
            "enabled": true,
            "agent": agent,
            "agent_error": null,
            "readiness": "ready",
            "model": strong,
            "model_choice": {"kind": "tier", "value": "strong"},
            "effort": "high",
            "skill": {"kind": "plugin", "id": "task-intake"},
            "kinds": [],
        })
    );
    let general = bot(&list, "general");
    assert_eq!((&general["model"], &general["effort"], &general["skill"]), (&json!(null), &json!(null), &json!(null)));
    assert_eq!(general["model_choice"], json!({"kind": "default"}));
    assert_eq!(general["kinds"], json!(["work"]));
    assert_eq!(
        (&bot(&list, "reviewer")["kinds"], &bot(&list, "planner")["kinds"]),
        (&json!(["review"]), &json!(["plan"]))
    );
}

#[test]
fn disabled_bots_are_listed_too() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    h.edit_bots(|bots| bots.iter_mut().find(|b| b.id == "setup").unwrap().enabled = false);
    assert_eq!(bot(&bots(&st, &h), "setup")["enabled"], false);
}

/// Git Operator e Release nascono spenti: il pianificatore li vede spenti (per
/// nominarli quando chiede all'utente di accenderli) e le sue istruzioni gli
/// vietano di proporli finche' restano spenti.
#[test]
fn the_preset_bots_that_start_disabled_are_listed_disabled_for_the_planner() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    h.edit_bots(|bots| *bots = crate::bots::default_agent_bots());
    let list = bots(&st, &h);
    let off: Vec<_> = list.iter().filter(|b| b["enabled"] == false).map(|b| b["id"].as_str().unwrap()).collect();
    assert_eq!(off, ["git-operator", "release"]);
    assert_eq!(bot(&list, "git-operator")["skill"], json!({"kind": "plugin", "id": "git-ops"}));
    assert_eq!(bot(&list, "release")["skill"], json!({"kind": "plugin", "id": "release-prep"}));
    let rule = "If the Bot you would need is disabled or not ready, do not propose: complete with the question for \
                the user, naming the Bot";
    assert!(crate::board::texts::PLANNER_INSTRUCTIONS.contains(rule));
}

#[test]
fn a_bot_whose_agent_cannot_be_used_reports_why() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    let mut w = h.ws_for("ws-1").unwrap();
    w.agent = Some("gone".into());
    h.store_ws1(w);
    let list = bots(&st, &h);
    let general = bot(&list, "general");
    assert_eq!(general["agent"], json!(null));
    assert_eq!(general["agent_error"], "unknown agent: gone");
    assert_eq!((&general["readiness"], &general["model"]), (&json!(null), &json!(null)));
}

#[test]
fn the_readiness_is_the_one_probed_for_the_agents_once_per_agent() {
    let (st, h) = (AutomationState::default(), FakeHost::default());
    let agent = registry::default_agent().id();
    h.set_readiness(agent, Some(Readiness::NotFound));
    let list = bots(&st, &h);
    assert!(list.iter().all(|b| b["readiness"] == "not-found"));
    let probes = h.probed().iter().filter(|a| a.as_str() == agent).count();
    assert_eq!(probes, 1, "one probe feeds both agents and bots");
}
