use super::*;
use crate::platform::TargetKind;
use crate::store::WarningKind;
use serde_json::json;
use std::path::PathBuf;

const FILE_NAME: &str = "workspaces.json";

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cc-workspaces-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn entry(id: &str) -> Value {
    json!({"id": id, "name": id, "kind": "windows", "path": format!("C:\\{id}"), "created_at": 1})
}

fn workspace(id: &str) -> Workspace {
    Workspace::new(id.into(), id.into(), TargetKind::Windows, format!("C:\\{id}"))
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("workspaces.json.bak-"))
        .collect()
}

fn read_json(dir: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()).unwrap()
}

fn ids(list: &[Workspace]) -> Vec<&str> {
    list.iter().map(|w| w.id.as_str()).collect()
}

/// Store per i test che non bloccano mai le scritture; chi le blocca ha il suo file (il blocco e' per istanza).
fn local_store() -> WorkspaceStore {
    static LOCAL: JsonFile<Vec<Workspace>> = workspaces_file();
    WorkspaceStore::with_file(&LOCAL)
}

#[test]
fn missing_file_gives_no_workspaces_and_no_warning() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("missing");
    let loaded = FILE.load(&dir);
    assert!(loaded.value.is_empty() && loaded.warning.is_none());
    assert!(!dir.join(FILE_NAME).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_old_file_without_the_envelope_is_read_migrated_and_rewritten_with_it() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("legacy");
    let old = json!([
        {"id":"a","name":"a","kind":"windows","path":"C:\\a","created_at":1,
         "auto_orchestrate":{"enabled":true},"worktree_setup":" npm ci ",
         "default_launch":"builtin:orchestrator",
         "launches":[{"id":"l1","name":"n","command":"ls","is_default":true}]},
        {"id":"b","name":"b","kind":"wsl","distro":"Ubuntu","path":"/home/b","created_at":2,
         "worker_permission":"plan","launches":[{"id":"l2","name":"n","command":"ls","is_default":true}]}
    ]);
    std::fs::write(dir.join(FILE_NAME), old.to_string()).unwrap();

    let loaded = FILE.load(&dir);
    assert!(loaded.warning.is_none(), "un file vecchio ma valido non e' un'anomalia");
    let [a, b] = &loaded.value[..] else { panic!("attesi due workspace") };
    assert!(a.use_schedule && a.repos["."].setup == ["npm ci"]);
    assert_eq!(
        a.default_launch.as_deref(),
        Some("builtin:preset:task-intake"),
        "il flag non batte il valore esistente"
    );
    assert_eq!(b.default_launch.as_deref(), Some("l2"));
    assert_eq!(b.worker.permission, crate::agents::PermissionMode::Plan);
    assert!(backups(&dir).is_empty());

    FILE.save(&dir, &loaded.value).unwrap();
    let raw = read_json(&dir);
    assert_eq!(raw["schema_version"], 1);
    let saved = &raw["data"][0];
    assert!(saved.get("auto_orchestrate").is_none() && saved.get("worktree_setup").is_none());
    assert!(saved["launches"][0].get("is_default").is_none());
    assert_eq!(FILE.load(&dir).value, loaded.value);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_written_json_keeps_the_historic_shape() {
    let dir = temp_dir("shape");
    let store = local_store();
    store
        .update(&dir, |list| {
            list.push(workspace("a"));
            Ok(())
        })
        .unwrap();
    let raw = read_json(&dir);
    assert_eq!(raw["schema_version"], 1);
    assert_eq!(
        raw["data"],
        json!([{
            "id": "a", "name": "a", "kind": "windows", "path": "C:\\a", "created_at": 0,
            "launches": [], "worker_permission": "accept-edits", "worker_model": null,
            "worker_allow": [], "worker_deny": [],
            "delivery": {"mode": "commit", "parallel": false, "merge_method": "squash"},
            "use_schedule": false
        }])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_workspace_among_valid_ones_is_backed_up_and_reported() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("partial");
    let broken = json!({"id": "broken", "name": "x", "kind": "linux", "path": "/x", "created_at": 1});
    let raw = json!([entry("a"), broken, entry("c"), 42]).to_string();
    std::fs::write(dir.join(FILE_NAME), &raw).unwrap();

    let loaded = FILE.load(&dir);
    assert_eq!(ids(&loaded.value), ["a", "c"], "gli altri workspace si caricano");
    let warning = loaded.warning.expect("serve un avviso");
    assert_eq!(warning.kind, WarningKind::PartiallyRead);
    assert!(
        warning.message.contains("workspace \"broken\"") && warning.message.contains("voce 4"),
        "{}",
        warning.message
    );
    let backup = warning.backup.expect("serve il backup");
    assert_eq!(std::fs::read_to_string(dir.join(backup)).unwrap(), raw, "il backup contiene anche le voci escluse");
    assert_eq!(backups(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_after_a_partial_read_drops_only_the_unreadable_ones() {
    let dir = temp_dir("partial-save");
    let raw = json!([entry("a"), {"id": 7}, entry("c")]).to_string();
    std::fs::write(dir.join(FILE_NAME), &raw).unwrap();
    let store = local_store();
    store.load(&dir);
    assert_eq!(ids(&store.all()), ["a", "c"]);
    store
        .modify(&dir, "c", |w| {
            w.name = "renamed".into();
            Ok(())
        })
        .unwrap();
    assert_eq!(read_json(&dir)["data"].as_array().unwrap().len(), 2);
    let backup = backups(&dir).pop().expect("backup");
    assert_eq!(std::fs::read_to_string(backup).unwrap(), raw);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_is_not_a_list_is_set_aside_whole() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("corrupt");
    std::fs::write(dir.join(FILE_NAME), r#"{"not":"a list"}"#).unwrap();
    let loaded = FILE.load(&dir);
    assert!(loaded.value.is_empty());
    assert_eq!(loaded.warning.unwrap().kind, WarningKind::Corrupt);
    assert!(!dir.join(FILE_NAME).exists());
    assert_eq!(backups(&dir).len(), 1);
    std::fs::write(dir.join(FILE_NAME), "{").unwrap();
    assert_eq!(FILE.load(&dir).warning.unwrap().kind, WarningKind::Corrupt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_from_a_newer_version_is_set_aside_not_overwritten() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("newer");
    let raw = json!({"schema_version": 99, "data": [entry("a")]}).to_string();
    std::fs::write(dir.join(FILE_NAME), &raw).unwrap();
    let loaded = FILE.load(&dir);
    assert!(loaded.value.is_empty());
    let warning = loaded.warning.unwrap();
    assert_eq!(warning.kind, WarningKind::NewerVersion);
    assert_eq!(std::fs::read_to_string(dir.join(warning.backup.unwrap())).unwrap(), raw);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_error_is_not_a_missing_file_and_blocks_saving() {
    static BLOCKED: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("unreadable");
    // Una cartella al posto del file: la lettura fallisce (non NotFound) ovunque.
    std::fs::create_dir_all(dir.join(FILE_NAME)).unwrap();
    let store = WorkspaceStore::with_file(&BLOCKED);
    store.load(&dir);
    assert!(store.all().is_empty());
    let err = store
        .update(&dir, |list| {
            list.push(workspace("a"));
            Ok(())
        })
        .unwrap_err();
    assert!(err.contains("workspaces.json"), "{err}");
    assert!(store.all().is_empty(), "lo stato in memoria resta com'era");
    assert!(dir.join(FILE_NAME).is_dir(), "il file non viene toccato");
    assert!(backups(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_read_error_warning_is_unreadable() {
    static FILE: JsonFile<Vec<Workspace>> = workspaces_file();
    let dir = temp_dir("unreadable-warning");
    std::fs::create_dir_all(dir.join(FILE_NAME)).unwrap();
    let warning = FILE.load(&dir).warning.unwrap();
    assert_eq!(warning.kind, WarningKind::Unreadable);
    assert!(warning.backup.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn modify_saves_and_returns_the_new_version() {
    let dir = temp_dir("modify");
    let store = local_store();
    store
        .update(&dir, |list| {
            list.extend([workspace("a"), workspace("b")]);
            Ok(())
        })
        .unwrap();
    let saved = store
        .modify(&dir, "b", |w| {
            w.name = "bee".into();
            Ok(())
        })
        .unwrap();
    assert_eq!(saved.name, "bee");
    assert_eq!(store.find("b"), Some(saved));
    let reloaded = local_store();
    reloaded.load(&dir);
    assert_eq!(reloaded.find("b").unwrap().name, "bee");
    assert_eq!(reloaded.find("a").unwrap().name, "a");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn modify_of_an_unknown_id_fails_without_writing() {
    let dir = temp_dir("modify-missing");
    let store = local_store();
    assert_eq!(store.modify(&dir, "ghost", |_| Ok(())).unwrap_err(), WORKSPACE_NOT_FOUND);
    assert!(!dir.join(FILE_NAME).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failing_modification_changes_neither_memory_nor_disk() {
    let dir = temp_dir("modify-fails");
    let store = local_store();
    store
        .update(&dir, |list| {
            list.push(workspace("a"));
            Ok(())
        })
        .unwrap();
    let before = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
    let err = store
        .modify(&dir, "a", |w| {
            w.name = "mezzo cambiato".into();
            Err("rifiutato".to_string())
        })
        .unwrap_err();
    assert_eq!(err, "rifiutato");
    assert_eq!(store.find("a").unwrap().name, "a");
    assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_returns_what_the_closure_computed() {
    let dir = temp_dir("update-value");
    let store = local_store();
    let count = store
        .update(&dir, |list| {
            list.push(workspace("a"));
            Ok(list.len())
        })
        .unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn find_and_all_see_the_current_list() {
    let dir = temp_dir("find");
    let store = local_store();
    assert!(store.find("a").is_none());
    store
        .update(&dir, |list| {
            list.extend([workspace("a"), workspace("b")]);
            Ok(())
        })
        .unwrap();
    assert_eq!(ids(&store.all()), ["a", "b"]);
    assert_eq!(store.find("a").unwrap().id, "a");
    assert!(store.find("zz").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn generated_ids_are_unique_even_within_the_same_millisecond() {
    let store = local_store();
    let ids: Vec<String> = (0..50).map(|_| store.next_id(WORKSPACE_ID_PREFIX)).collect();
    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
    assert!(ids[0].starts_with("ws-") && ids[0].ends_with("-1"), "{}", ids[0]);
    assert!(store.next_id(LAUNCH_ID_PREFIX).starts_with("ln-"));
}
