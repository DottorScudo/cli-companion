//! Migrazioni del file `workspaces.json`: passi puri sul JSON, uno per salto di
//! versione dello schema (vedi `store::migrate`).

use serde_json::{json, Value};

/// Chiave del repository che coincide con la root del workspace.
const ROOT_REPO_KEY: &str = ".";

/// Errore di un file che non e' un elenco di workspace (anche per la lettura).
pub(super) const NOT_A_LIST: &str = "atteso un elenco di workspace";

/// Passo 0 -> 1: introduce la busta e porta il formato storico a quello
/// attuale, workspace per workspace:
/// - `auto_orchestrate.enabled` -> `use_schedule`;
/// - `worktree_setup` -> `repos["."].setup` (se questo e' vuoto);
/// - il flag `is_default` degli avvii -> `default_launch`;
/// - l'avvio rapido `builtin:orchestrator` (v0.11) -> `builtin:preset:task-intake`.
///
/// Le voci che non sono oggetti restano com'erano: le scarta la lettura.
pub(super) fn migrate_v0(mut data: Value) -> Result<Value, String> {
    let Value::Array(workspaces) = &mut data else { return Err(NOT_A_LIST.to_string()) };
    workspaces.iter_mut().filter_map(Value::as_object_mut).for_each(|ws| {
        migrate_auto_orchestrate(ws);
        migrate_worktree_setup(ws);
        migrate_default_launch(ws);
    });
    Ok(data)
}

type Object = serde_json::Map<String, Value>;

fn migrate_auto_orchestrate(ws: &mut Object) {
    let enabled =
        ws.remove("auto_orchestrate").and_then(|a| a.get("enabled").and_then(Value::as_bool)).unwrap_or(false);
    if enabled {
        ws.insert("use_schedule".into(), true.into());
    }
}

fn migrate_worktree_setup(ws: &mut Object) {
    let setup = ws.remove("worktree_setup").and_then(|v| v.as_str().map(|c| c.trim().to_string()));
    let Some(cmd) = setup.filter(|c| !c.is_empty()) else { return };
    let repos = ws.entry("repos").or_insert_with(|| json!({}));
    let Some(root) = repos.as_object_mut().map(|map| map.entry(ROOT_REPO_KEY).or_insert_with(|| json!({}))) else {
        return;
    };
    if let Some(root) = root.as_object_mut() {
        let empty = root.get("setup").and_then(Value::as_array).is_none_or(Vec::is_empty);
        if empty {
            root.insert("setup".into(), json!([cmd]));
        }
    }
}

fn migrate_default_launch(ws: &mut Object) {
    let flagged = ws.get("launches").and_then(Value::as_array).and_then(|launches| {
        launches
            .iter()
            .find(|l| l.get("is_default").and_then(Value::as_bool) == Some(true))
            .and_then(|l| l.get("id").and_then(Value::as_str).map(str::to_string))
    });
    let already_set = ws.get("default_launch").is_some_and(|v| !v.is_null());
    if !already_set {
        if let Some(id) = flagged {
            ws.insert("default_launch".into(), Value::String(id));
        }
    }
    if let Some(launches) = ws.get_mut("launches").and_then(Value::as_array_mut) {
        launches.iter_mut().filter_map(Value::as_object_mut).for_each(|l| {
            l.remove("is_default");
        });
    }
    if let Some(slot) = ws.get_mut("default_launch") {
        let mut id = slot.as_str().map(str::to_string);
        crate::agents::presets::migrate_launch_id(&mut id);
        if let Some(id) = id {
            *slot = Value::String(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrate(v: Value) -> Value {
        migrate_v0(v).unwrap()
    }

    #[test]
    fn legacy_fields_are_migrated_and_dropped() {
        let out = migrate(json!([
            {"id":"a","auto_orchestrate":{"enabled":true,"agent":"claude-code"},"worktree_setup":"  npm ci "},
            {"id":"b","auto_orchestrate":{"enabled":false},"worktree_setup":"npm ci","repos":{".":{"setup":["make"]}}},
            {"id":"c"}
        ]));
        assert_eq!(out[0], json!({"id":"a","use_schedule":true,"repos":{".":{"setup":["npm ci"]}}}));
        // `worktree_setup` diventa il setup della root solo se questo e' vuoto.
        assert_eq!(out[1], json!({"id":"b","repos":{".":{"setup":["make"]}}}));
        assert_eq!(out[2], json!({"id":"c"}));
    }

    #[test]
    fn blank_worktree_setup_is_just_dropped() {
        assert_eq!(migrate(json!([{"id":"a","worktree_setup":"   "}])), json!([{"id":"a"}]));
        let kept = migrate(json!([{"id":"a","worktree_setup":"x","repos":{".":{"setup":[]}}}]));
        assert_eq!(kept[0]["repos"]["."]["setup"], json!(["x"]));
        // `repos` malformato: non si tocca e non va in panico.
        assert_eq!(migrate(json!([{"id":"a","worktree_setup":"x","repos":3}]))[0]["repos"], json!(3));
    }

    #[test]
    fn is_default_flag_moves_to_the_workspace() {
        let out = migrate(json!([{
            "id":"a",
            "launches":[{"id":"l1","name":"n","command":"c"},{"id":"l2","name":"n","command":"c","is_default":true}]
        }]));
        assert_eq!(out[0]["default_launch"], "l2");
        assert!(out[0]["launches"].as_array().unwrap().iter().all(|l| l.get("is_default").is_none()));
        // Un valore gia' presente non viene sovrascritto.
        let out = migrate(json!([{"id":"a","default_launch":"shell","launches":[{"id":"l1","is_default":true}]}]));
        assert_eq!(out[0]["default_launch"], "shell");
        // Nessun flag: resta assente.
        let out = migrate(json!([{"id":"a","launches":[{"id":"l1","is_default":false}]}]));
        assert!(out[0].get("default_launch").is_none());
    }

    #[test]
    fn the_orchestrator_launch_becomes_task_intake() {
        let out = migrate(json!([
            {"id":"a","default_launch":"builtin:orchestrator"},
            {"id":"b","default_launch":"builtin:claude-code"},
            {"id":"c","default_launch":null}
        ]));
        assert_eq!(out[0]["default_launch"], "builtin:preset:task-intake");
        assert_eq!(out[1]["default_launch"], "builtin:claude-code");
        assert_eq!(out[2]["default_launch"], Value::Null);
    }

    #[test]
    fn running_twice_changes_nothing_more() {
        let once = migrate(json!([{
            "id":"a","auto_orchestrate":{"enabled":true},"worktree_setup":"x",
            "default_launch":"builtin:orchestrator","launches":[{"id":"l1","is_default":true}]
        }]));
        assert_eq!(migrate(once.clone()), once);
    }

    #[test]
    fn entries_that_are_not_objects_are_left_alone() {
        let input = json!([1, "x", null, {"id":"ok"}]);
        assert_eq!(migrate(input.clone()), input);
    }

    #[test]
    fn data_that_is_not_a_list_is_rejected() {
        for bad in [json!({}), json!("x"), json!(null), json!(3)] {
            assert_eq!(migrate_v0(bad).unwrap_err(), NOT_A_LIST);
        }
    }
}
