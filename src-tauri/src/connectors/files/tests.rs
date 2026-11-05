use super::*;
use crate::connectors::limits::SEEN_MAX;
use crate::connectors::model::{ConnectorNotices, LinkThread, RouteThread, RouteWake};
use crate::connectors::state::MAIN_CURSOR;
use crate::hub::Channel;
use crate::store::WarningKind;
use serde_json::json;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-connectors-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn instance(id: &str, connector: &str) -> ConnectorInstance {
    ConnectorInstance {
        id: id.to_string(),
        connector: connector.to_string(),
        label: "Etichetta".to_string(),
        enabled: false,
        config: json!({"mailbox": "x"}),
        config_version: 1,
        notices: ConnectorNotices::default(),
        created_at: 1,
        updated_at: 2,
    }
}

fn route(id: &str, instance: &str) -> ConnectorRoute {
    ConnectorRoute {
        id: id.to_string(),
        instance: instance.to_string(),
        label: "Regola".to_string(),
        filter: json!({"query": "from:a"}),
        channel: Channel::General,
        thread: RouteThread::Existing { thread_id: "thr-1-1".to_string() },
        wake: RouteWake::Secretary("mario".to_string()),
        enabled: true,
        error: None,
        created_at: 1,
        updated_at: 2,
    }
}

fn link(id: &str, instance: &str) -> ConnectorLink {
    ConnectorLink {
        id: id.to_string(),
        instance: instance.to_string(),
        external_account: "42".to_string(),
        external_chat: "42".to_string(),
        display_name: "Mario".to_string(),
        current: Some(LinkThread { channel: Channel::General, thread_id: "thr-1-1".to_string() }),
        channel: None,
        muted_until: None,
        created_at: 3,
    }
}

fn sample() -> ConnectorsFile {
    ConnectorsFile {
        instances: vec![instance("con-1-1", "fake-source"), instance("con-1-2", "fake-contact")],
        routes: vec![route("rte-1-1", "con-1-1")],
        links: vec![link("lnk-1-1", "con-1-2")],
    }
}

fn write_raw(dir: &Path, data: &Value) {
    let raw = json!({"schema_version": 1, "data": data});
    std::fs::write(dir.join("connectors.json"), serde_json::to_string_pretty(&raw).unwrap()).unwrap();
}

fn backups(dir: &Path) -> Vec<String> {
    names(dir).into_iter().filter(|n| n.starts_with("connectors.json.bak-")).collect()
}

#[test]
fn a_missing_file_gives_defaults_and_creates_nothing() {
    let dir = temp_dir("missing");
    let file = connectors_file();
    let loaded = file.load(&dir);
    assert_eq!(loaded.value, ConnectorsFile::default());
    assert!(loaded.warning.is_none());
    assert!(names(&dir).is_empty());
    let state = StateFile::open(&dir, "con-1-1").unwrap();
    assert_eq!(state.load().value, InstanceState::default());
    assert!(names(&dir).is_empty(), "nemmeno la cartella dello stato nasce da una lettura");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_roundtrips_in_the_store_envelope() {
    let dir = temp_dir("roundtrip");
    let file = connectors_file();
    file.save(&dir, &sample()).unwrap();
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("connectors.json")).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 1);
    assert_eq!(raw["data"]["routes"][0]["wake"], "secretary:mario");
    assert_eq!(raw["data"]["routes"][0]["thread"], json!({"mode": "existing", "thread_id": "thr-1-1"}));
    assert_eq!(raw["data"]["routes"][0]["channel"], "general");
    let loaded = file.load(&dir);
    assert!(loaded.warning.is_none());
    assert_eq!(loaded.value, sample());
    assert_eq!(names(&dir), vec!["connectors.json".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_entry_is_skipped_with_a_backup_and_the_others_are_kept() {
    let dir = temp_dir("partial");
    let mut data = serde_json::to_value(sample()).unwrap();
    data["instances"].as_array_mut().unwrap().push(json!({"id": "con-9-9", "connector": 5}));
    data["instances"].as_array_mut().unwrap().push(json!({"id": "bad id", "connector": "x", "label": "L", "enabled": false, "config": {}, "config_version": 1, "created_at": 1, "updated_at": 1}));
    data["routes"].as_array_mut().unwrap().push(json!({"id": "rte-9-9"}));
    write_raw(&dir, &data);
    let loaded = connectors_file().load(&dir);
    assert_eq!(loaded.value, sample());
    let warning = loaded.warning.unwrap();
    assert_eq!(warning.kind, WarningKind::PartiallyRead);
    assert!(warning.message.contains("con-9-9") && warning.message.contains("3 voci"), "{}", warning.message);
    assert_eq!(backups(&dir).len(), 1);
    assert!(dir.join("connectors.json").is_file(), "il file originale resta finche' non si salva");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_connector_instance_is_kept_untouched_without_warnings() {
    let dir = temp_dir("unknown");
    let mut instance = instance("con-1-1", "connettore-futuro");
    instance.config = json!({"campo_nuovo": [1, 2, {"x": null}], "altro": "v"});
    instance.config_version = 7;
    instance.enabled = true;
    let original =
        ConnectorsFile { instances: vec![instance.clone()], routes: vec![route("rte-1-1", "con-1-1")], links: vec![] };
    write_raw(&dir, &serde_json::to_value(&original).unwrap());
    let file = connectors_file();
    let loaded = file.load(&dir);
    assert!(loaded.warning.is_none());
    assert!(backups(&dir).is_empty());
    assert_eq!(loaded.value, original);
    // Risalvato insieme ad altro: l'istanza sconosciuta resta identica.
    let mut next = loaded.value;
    next.instances.push(self::instance("con-1-2", "fake-source"));
    file.save(&dir, &next).unwrap();
    let again = file.load(&dir).value;
    assert_eq!(again.instances[0], instance);
    assert_eq!(again.routes, original.routes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicates_and_orphans_are_excluded_and_reported() {
    let dir = temp_dir("orphans");
    let data = json!({
        "instances": [instance("con-1-1", "fake-source"), instance("con-1-1", "fake-source")],
        "routes": [route("rte-1-1", "con-1-1"), route("rte-1-2", "con-5-5")],
        "links": [link("lnk-1-1", "con-6-6")]
    });
    write_raw(&dir, &serde_json::to_value(data).unwrap());
    let loaded = connectors_file().load(&dir);
    assert_eq!(loaded.value.instances.len(), 1);
    assert_eq!(loaded.value.routes.len(), 1);
    assert!(loaded.value.links.is_empty());
    let message = loaded.warning.unwrap().message;
    assert!(message.contains("duplicato") && message.contains("istanza inesistente"), "{message}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_of_the_wrong_shape_is_set_aside_and_never_destroyed() {
    let dir = temp_dir("shape");
    for (name, data) in [("list", json!([1, 2])), ("section", json!({"instances": {"a": 1}}))] {
        write_raw(&dir, &data);
        let file = connectors_file();
        let loaded = file.load(&dir);
        assert_eq!(loaded.value, ConnectorsFile::default(), "{name}");
        let warning = loaded.warning.unwrap();
        assert_eq!(warning.kind, WarningKind::Corrupt, "{name}");
        assert!(!dir.join("connectors.json").exists());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(backups(&dir).len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_schema_is_set_aside() {
    let dir = temp_dir("newer");
    std::fs::write(dir.join("connectors.json"), r#"{"schema_version":99,"data":{}}"#).unwrap();
    let warning = connectors_file().load(&dir).warning.unwrap();
    assert_eq!(warning.kind, WarningKind::NewerVersion);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_never_contains_secrets_or_unknown_fields_of_the_dtos() {
    let text = serde_json::to_string(&sample()).unwrap().replace("secretary", "");
    for forbidden in ["secret", "token", "password"] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn state_files_roundtrip_one_per_instance() {
    let dir = temp_dir("state");
    let a = StateFile::open(&dir, "con-1-1").unwrap();
    let b = StateFile::open(&dir, "con-1-2").unwrap();
    let mut state = InstanceState::default();
    state.set_cursor(MAIN_CURSOR, json!({"offset": 3}));
    state.mark_seen("rte-1-1", "m1");
    a.save(&state).unwrap();
    assert_eq!(a.load().value, state);
    assert_eq!(b.load().value, InstanceState::default());
    assert_eq!(names(&dir.join("connector_state")), vec!["con-1-1.json".to_string()]);
    a.delete().unwrap();
    a.delete().unwrap();
    assert!(names(&dir.join("connector_state")).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn state_ids_must_be_instance_ids() {
    let dir = temp_dir("stateid");
    for bad in ["", "../con-1-1", "con-1-1/x", "rte-1-1", "con-1"] {
        assert!(StateFile::open(&dir, bad).is_err(), "{bad:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_corrupt_state_file_is_backed_up_and_the_instance_restarts_from_now() {
    let dir = temp_dir("statecorrupt");
    let state_dir = dir.join("connector_state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::write(state_dir.join("con-1-1.json"), "{ rotto").unwrap();
    let file = StateFile::open(&dir, "con-1-1").unwrap();
    let loaded = file.load();
    assert_eq!(loaded.value, InstanceState::default());
    assert!(loaded.value.cursor(MAIN_CURSOR).is_none(), "nessun cursore = si riparte da adesso");
    let warning = loaded.warning.unwrap();
    assert_eq!(warning.kind, WarningKind::Corrupt);
    let backup = warning.backup.unwrap();
    assert_eq!(std::fs::read_to_string(state_dir.join(backup)).unwrap(), "{ rotto");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_oversized_state_file_is_bounded_on_load() {
    let dir = temp_dir("statebig");
    let state_dir = dir.join("connector_state");
    std::fs::create_dir_all(&state_dir).unwrap();
    let ids: Vec<String> = (0..SEEN_MAX + 100).map(|i| format!("r:{i}")).collect();
    let raw = json!({"schema_version": 1, "data": {"seen": ids}});
    std::fs::write(state_dir.join("con-1-1.json"), raw.to_string()).unwrap();
    let state = StateFile::open(&dir, "con-1-1").unwrap().load().value;
    assert_eq!(state.seen.len(), SEEN_MAX);
    assert!(!state.seen.contains("r:99") && state.seen.contains("r:100"));
    let _ = std::fs::remove_dir_all(&dir);
}
