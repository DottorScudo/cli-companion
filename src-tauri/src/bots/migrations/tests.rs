use super::*;
use crate::bots::presets::default_bots;
use crate::bots::{AgentBot, BotSettings};

fn ids(value: &Value) -> Vec<&str> {
    value[AGENTS_KEY].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect()
}

#[test]
fn the_v2_step_adds_the_four_presets_and_keeps_the_rest() {
    let v1 = json!({"enabled": false, "names": ["Ada"], "images": {"Ada": {"png": "AAAA", "color": "#112233"}}});
    let v2 = add_agent_bots(v1.clone()).unwrap();
    assert_eq!(ids(&v2), ["general", "intake", "setup", "skill-wizard"]);
    for key in ["enabled", "names", "images"] {
        assert_eq!(v2[key], v1[key], "{key}");
    }
}

/// Oggi i passi congelati, uno dopo l'altro, e i preset coincidono. Se un
/// preset cambia, i passi restano come sono e questo test si aggiorna con
/// un passo nuovo in coda.
#[test]
fn the_steps_match_the_presets_of_the_current_release() {
    let steps = [
        add_agent_bots,
        add_task_kinds,
        add_answer_and_fix_bots,
        exclusive_general_and_planner_tasks,
        add_research_and_work_bots,
        add_git_and_release_bots,
    ];
    let current = steps.iter().try_fold(json!({}), |value, step| step(value)).unwrap();
    let bots: Vec<AgentBot> = serde_json::from_value(current[AGENTS_KEY].clone()).unwrap();
    assert_eq!(bots, default_bots());
    let settings: BotSettings = serde_json::from_value(current).unwrap();
    assert_eq!(settings.agents, default_bots());
}

#[test]
fn present_ids_are_skipped_and_others_are_untouched() {
    let mine = json!({"id": "intake", "name": "My intake"});
    let v2 = add_agent_bots(json!({"agents": [mine.clone()]})).unwrap();
    assert_eq!(ids(&v2), ["intake", "general", "setup", "skill-wizard"]);
    assert_eq!(v2[AGENTS_KEY][0], mine);
}

#[test]
fn malformed_data_is_an_error() {
    for step in [
        add_agent_bots,
        add_task_kinds,
        add_answer_and_fix_bots,
        exclusive_general_and_planner_tasks,
        add_research_and_work_bots,
        add_git_and_release_bots,
    ] {
        assert!(step(json!([1, 2])).is_err());
        assert!(step(json!({"agents": "nope"})).is_err());
    }
    assert!(add_task_kinds(json!({"agents": [1]})).is_err());
    assert!(exclusive_general_and_planner_tasks(json!({"agents": [1]})).is_err());
}

fn kinds_of<'a>(value: &'a Value, id: &str) -> &'a Value {
    let list = value[AGENTS_KEY].as_array().unwrap();
    &list.iter().find(|b| b["id"] == id).unwrap()[KINDS_KEY]
}

#[test]
fn the_v3_step_gives_each_kind_of_bot_its_task_kinds() {
    let bot = |id: &str| json!({"id": id, "name": id});
    let ids_v2 = ["general", "intake", "setup", "skill-wizard", "intake-other", "custom-mine"];
    let v2 = json!({"agents": ids_v2.map(bot)});
    let v3 = add_task_kinds(v2).unwrap();
    assert_eq!(kinds_of(&v3, "general"), &json!(["work"]));
    for id in ["intake", "setup", "skill-wizard"] {
        assert_eq!(kinds_of(&v3, id), &json!([]), "{id}");
    }
    // Derivati e Bot dell'utente: nessun cambio di comportamento.
    for id in ["intake-other", "custom-mine"] {
        assert_eq!(kinds_of(&v3, id), &json!(ALL_KINDS), "{id}");
    }
    assert_eq!(kinds_of(&v3, "reviewer"), &json!(["review"]));
    assert_eq!(kinds_of(&v3, "planner"), &json!(["plan"]));
}

#[test]
fn the_v3_step_keeps_the_rest_and_does_not_bring_back_deleted_presets() {
    let v2 = json!({"enabled": false, "names": ["Ada"], "agents": [{"id": "general", "name": "General"}]});
    let v3 = add_task_kinds(v2.clone()).unwrap();
    assert_eq!(ids(&v3), ["general", "reviewer", "planner"]);
    assert_eq!((&v3["enabled"], &v3["names"]), (&v2["enabled"], &v2["names"]));
    assert_eq!(v3[AGENTS_KEY][0]["name"], "General");
}

#[test]
fn the_v3_step_skips_present_ids_and_frees_taken_names() {
    let mine = json!({"id": "planner", "name": "Mine", "kinds": ["work"]});
    let v2 = json!({"agents": [mine.clone(), {"id": "custom-r", "name": "reviewer"}, {"id": "custom-r2", "name": "Reviewer 2"}]});
    let v3 = add_task_kinds(v2).unwrap();
    assert_eq!(ids(&v3), ["planner", "custom-r", "custom-r2", "reviewer"]);
    assert_eq!(v3[AGENTS_KEY][0], mine, "kinds already set are kept");
    assert_eq!(v3[AGENTS_KEY][3]["name"], "Reviewer 3");
}

#[test]
fn the_v4_step_adds_the_two_bots_at_the_end_and_keeps_the_rest() {
    let mine = json!({"id": "custom-mine", "name": "Mine", "kinds": ["work"]});
    let v3 = json!({"enabled": false, "names": ["Ada"], "agents": [{"id": "general", "name": "General"}, mine]});
    let v4 = add_answer_and_fix_bots(v3.clone()).unwrap();
    assert_eq!(ids(&v4), ["general", "custom-mine", "quick-answer", "bug-fixer"]);
    assert_eq!(v4[AGENTS_KEY].as_array().unwrap()[..2], v3[AGENTS_KEY].as_array().unwrap()[..]);
    assert_eq!((&v4["enabled"], &v4["names"]), (&v3["enabled"], &v3["names"]));
    assert_eq!(kinds_of(&v4, "quick-answer"), &json!(["plan"]));
    assert_eq!(kinds_of(&v4, "bug-fixer"), &json!(["work"]));
}

#[test]
fn the_v4_step_skips_present_ids_and_frees_taken_names() {
    let mine = json!({"id": "bug-fixer", "name": "Mine"});
    let v3 = json!({"agents": [mine.clone(), {"id": "custom-q", "name": "quick answer"}]});
    let v4 = add_answer_and_fix_bots(v3).unwrap();
    assert_eq!(ids(&v4), ["bug-fixer", "custom-q", "quick-answer"]);
    assert_eq!(v4[AGENTS_KEY][0], mine);
    assert_eq!(v4[AGENTS_KEY][2]["name"], "Quick Answer 2");
}

#[test]
fn the_v5_step_changes_only_the_task_of_general_and_planner() {
    let old = "General-purpose coding agent.";
    let general =
        json!({"id": "general", "name": "Mine", "task": old, "model": {"kind": "default"}, "kinds": ["work"]});
    let derived = json!({"id": "general-other", "name": "General (Other)", "task": old, "kinds": ["work"]});
    let v4 = json!({"enabled": false, "names": ["Ada"], "agents": [general.clone(), derived.clone()]});
    let v5 = exclusive_general_and_planner_tasks(v4.clone()).unwrap();
    let mut expected = general;
    expected[TASK_KEY] = json!(crate::bots::presets::find(GENERAL).unwrap().task);
    assert_eq!(v5[AGENTS_KEY], json!([expected, derived]), "only the task of the general id changes");
    assert_eq!((&v5["enabled"], &v5["names"]), (&v4["enabled"], &v4["names"]));
}

#[test]
fn the_v6_step_adds_the_four_bots_at_the_end_and_keeps_the_rest() {
    let mine = json!({"id": "custom-mine", "name": "Mine", "kinds": ["work"]});
    let v5 = json!({"enabled": false, "names": ["Ada"], "agents": [{"id": "general", "name": "General"}, mine]});
    let v6 = add_research_and_work_bots(v5.clone()).unwrap();
    assert_eq!(ids(&v6), ["general", "custom-mine", "researcher", "test-writer", "docs-writer", "refactorer"]);
    assert_eq!(v6[AGENTS_KEY].as_array().unwrap()[..2], v5[AGENTS_KEY].as_array().unwrap()[..]);
    assert_eq!((&v6["enabled"], &v6["names"]), (&v5["enabled"], &v5["names"]));
    assert_eq!(kinds_of(&v6, "researcher"), &json!(["plan"]));
    for id in ["test-writer", "docs-writer", "refactorer"] {
        assert_eq!(kinds_of(&v6, id), &json!(["work"]), "{id}");
    }
}

#[test]
fn the_v6_step_skips_present_ids_frees_taken_names_and_is_idempotent() {
    let mine = json!({"id": "refactorer", "name": "Mine"});
    let v5 = json!({"agents": [mine.clone(), {"id": "custom-r", "name": "researcher"}]});
    let v6 = add_research_and_work_bots(v5).unwrap();
    assert_eq!(ids(&v6), ["refactorer", "custom-r", "researcher", "test-writer", "docs-writer"]);
    assert_eq!(v6[AGENTS_KEY][0], mine);
    assert_eq!(v6[AGENTS_KEY][2]["name"], "Researcher 2");
    assert_eq!(add_research_and_work_bots(v6.clone()).unwrap(), v6);
}

#[test]
fn the_v7_step_adds_the_two_bots_disabled_at_the_end_and_keeps_the_rest() {
    let mine = json!({"id": "custom-mine", "name": "Mine", "kinds": ["work"], "enabled": true});
    let v6 = json!({"enabled": false, "names": ["Ada"], "agents": [{"id": "general", "name": "General"}, mine]});
    let v7 = add_git_and_release_bots(v6.clone()).unwrap();
    assert_eq!(ids(&v7), ["general", "custom-mine", "git-operator", "release"]);
    assert_eq!(v7[AGENTS_KEY].as_array().unwrap()[..2], v6[AGENTS_KEY].as_array().unwrap()[..]);
    assert_eq!((&v7["enabled"], &v7["names"]), (&v6["enabled"], &v6["names"]));
    for id in ["git-operator", "release"] {
        assert_eq!(kinds_of(&v7, id), &json!(["work"]), "{id}");
        let bot = v7[AGENTS_KEY].as_array().unwrap().iter().find(|b| b["id"] == id).unwrap();
        assert_eq!(bot["enabled"], false, "{id} starts disabled");
    }
}

#[test]
fn the_v7_step_skips_present_ids_frees_taken_names_and_is_idempotent() {
    let mine = json!({"id": "git-operator", "name": "Mine", "enabled": true});
    let v6 = json!({"agents": [mine.clone(), {"id": "custom-r", "name": "release"}]});
    let v7 = add_git_and_release_bots(v6).unwrap();
    assert_eq!(ids(&v7), ["git-operator", "custom-r", "release"]);
    assert_eq!(v7[AGENTS_KEY][0], mine, "a present bot keeps its enabled");
    assert_eq!(v7[AGENTS_KEY][2]["name"], "Release 2");
    assert_eq!(add_git_and_release_bots(v7.clone()).unwrap(), v7);
}
