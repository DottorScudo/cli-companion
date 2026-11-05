use super::*;
use crate::connectors::commands::support::{contact_file, fixture, fixture_with, source_file};
use crate::connectors::model::{ConnectorsFile, RouteThread, RouteWake};
use crate::connectors::runtime::health::HealthState;
use crate::connectors::testing::{link, route, INSTANCE};
use crate::secrets::{MemorySecrets, SecretStore};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(5), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn status(rt: &ConnectorsRuntime, id: &str) -> Option<HealthState> {
    rt.status_registry().get(id).map(|s| s.state)
}

#[test]
fn an_instance_is_created_disabled_with_a_validated_config() {
    let f = fixture(ConnectorsFile::default());
    let created = create_instance(&f.rt, "fake-source", "  Posta  ", json!({"mailbox": " m "})).unwrap();
    assert!(!created.enabled && created.label == "Posta");
    assert_eq!(created.config, json!({"mailbox": "m"}));
    assert_eq!(created.config_version, 2);
    assert!(crate::connectors::ids::valid_instance_id(&created.id));
    assert_eq!(f.rt.snapshot().instances, vec![created.clone()]);
    assert_eq!(status(&f.rt, &created.id), Some(HealthState::Disabled));
    f.rt.set_hub_enabled(true);
    assert!(f.rt.running_ids().is_empty(), "creata spenta: nessun task");
}

#[test]
fn creation_rejects_unknown_connectors_secrets_and_bad_values_without_saving() {
    let f = fixture(ConnectorsFile::default());
    assert!(create_instance(&f.rt, "non-esiste", "x", json!({})).is_err());
    assert!(create_instance(&f.rt, "../x", "x", json!({})).is_err());
    assert!(create_instance(&f.rt, "fake-source", "", json!({})).is_err());
    let secret = create_instance(&f.rt, "fake-source", "x", json!({"api_token": "tok-SEGRETO"})).unwrap_err();
    assert!(!secret.contains("SEGRETO"), "{secret}");
    assert!(create_instance(&f.rt, "fake-source", "x", json!({"mailbox": "vietata"})).is_err());
    assert!(f.rt.snapshot().instances.is_empty());
}

#[test]
fn at_most_twenty_instances() {
    let f = fixture(ConnectorsFile::default());
    for n in 0..20 {
        create_instance(&f.rt, "fake-source", &format!("i{n}"), json!({})).unwrap();
    }
    let err = create_instance(&f.rt, "fake-source", "21", json!({})).unwrap_err();
    assert!(err.contains("20"), "{err}");
    assert_eq!(f.rt.snapshot().instances.len(), 20);
}

#[test]
fn update_revalidates_and_a_config_change_restarts_the_instance() {
    let mut file = source_file();
    file.instances[0].enabled = true;
    let f = fixture(file);
    f.rt.set_hub_enabled(true);
    wait_for("avviata", || status(&f.rt, INSTANCE).is_some_and(|s| s != HealthState::Disabled));
    let updated = update_instance(&f.rt, INSTANCE, "Nuova", json!({"mailbox": "b"})).unwrap();
    assert_eq!((updated.label.as_str(), updated.config.clone()), ("Nuova", json!({"mailbox": "b"})));
    assert_eq!(f.rt.running_ids(), vec![INSTANCE.to_string()], "riavviata");
    assert!(update_instance(&f.rt, INSTANCE, "x", json!({"mailbox": "vietata"})).is_err());
    assert!(update_instance(&f.rt, INSTANCE, "x", json!({"api_token": "SEGRETO"})).is_err());
    assert!(update_instance(&f.rt, "con-9-9", "x", json!({})).is_err());
    assert_eq!(f.rt.store().instance(INSTANCE).unwrap().config, json!({"mailbox": "b"}), "invariata dopo gli errori");
    f.rt.stop_all();
}

#[test]
fn a_config_of_an_older_version_is_not_editable() {
    let f = fixture(source_file());
    let mut file = f.rt.snapshot();
    file.instances[0].config_version = 1;
    f.rt.store().replace(file);
    let err = update_instance(&f.rt, INSTANCE, "x", json!({})).unwrap_err();
    assert!(err.contains("versione"), "{err}");
}

#[test]
fn enabling_requires_a_compiled_connector() {
    let mut file = source_file();
    let mut unknown = file.instances[0].clone();
    unknown.id = "con-1700000000000-2".to_string();
    unknown.connector = "sparito".to_string();
    file.instances.push(unknown);
    let f = fixture(file);
    let enabled = set_enabled(&f.rt, INSTANCE, true).unwrap();
    assert!(enabled.enabled);
    let err = set_enabled(&f.rt, "con-1700000000000-2", true).unwrap_err();
    assert!(err.contains("non disponibile"), "{err}");
    assert!(!f.rt.store().instance("con-1700000000000-2").unwrap().enabled);
    assert!(!set_enabled(&f.rt, "con-1700000000000-2", false).unwrap().enabled);
    assert!(set_enabled(&f.rt, "con-9-9", true).is_err());
}

#[test]
fn enabling_with_the_hub_on_starts_and_disabling_stops() {
    let f = fixture(source_file());
    f.rt.set_hub_enabled(true);
    assert_eq!(status(&f.rt, INSTANCE), Some(HealthState::Disabled));
    set_enabled(&f.rt, INSTANCE, true).unwrap();
    assert_eq!(f.rt.running_ids(), vec![INSTANCE.to_string()]);
    set_enabled(&f.rt, INSTANCE, false).unwrap();
    assert!(f.rt.running_ids().is_empty());
    assert_eq!(status(&f.rt, INSTANCE), Some(HealthState::Disabled));
}

fn full_file() -> ConnectorsFile {
    let mut file = source_file();
    file.routes.push(route("rte-1-1", RouteThread::NewPerItem, RouteWake::None));
    file.links.push(link("lnk-1-1", "acc", None));
    file
}

#[test]
fn deleting_removes_everything_of_the_instance() {
    let f = fixture(full_file());
    f.rt.instance_secrets(INSTANCE).unwrap().set("api_token", "tok-1").unwrap();
    f.rt.instance_secrets(INSTANCE).unwrap().set("refresh_token", "tok-2").unwrap();
    let mut deleted_state = Vec::new();
    let out = delete_instance(&f.rt, INSTANCE, |id| {
        deleted_state.push(id.to_string());
        Ok(())
    })
    .unwrap();
    assert_eq!(out.warning, None);
    assert_eq!(deleted_state, vec![INSTANCE.to_string()]);
    let file = f.rt.snapshot();
    assert!(file.instances.is_empty() && file.routes.is_empty() && file.links.is_empty());
    assert_eq!(f.rt.status_registry().get(INSTANCE), None);
    let secrets = f.rt.instance_secrets(INSTANCE).unwrap();
    assert!(!secrets.is_set("api_token").unwrap() && !secrets.is_set("refresh_token").unwrap());
    assert!(delete_instance(&f.rt, INSTANCE, |_| Ok(())).is_err(), "gia' eliminata");
}

#[test]
fn deleting_a_running_instance_stops_its_task() {
    let mut file = full_file();
    file.instances[0].enabled = true;
    let f = fixture(file);
    f.rt.set_hub_enabled(true);
    assert_eq!(f.rt.running_ids().len(), 1);
    delete_instance(&f.rt, INSTANCE, |_| Ok(())).unwrap();
    assert!(f.rt.running_ids().is_empty());
}

struct BrokenKeyring(MemorySecrets);

impl SecretStore for BrokenKeyring {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        self.0.get(account)
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0.set(account, secret)
    }

    fn delete(&self, _: &str) -> Result<(), String> {
        Err("portachiavi guasto".to_string())
    }
}

#[test]
fn a_keyring_or_state_error_is_reported_but_the_instance_is_removed_anyway() {
    let f = fixture_with(full_file(), Arc::new(BrokenKeyring(MemorySecrets::default())));
    let out = delete_instance(&f.rt, INSTANCE, |_| Err("disco pieno".to_string())).unwrap();
    let warning = out.warning.expect("avviso");
    assert!(warning.contains("portachiavi") && warning.contains("stato"), "{warning}");
    assert!(!warning.contains("disco pieno"));
    assert!(f.rt.snapshot().instances.is_empty());
}

#[test]
fn secrets_are_set_and_cleared_only_for_declared_slots() {
    let f = fixture(source_file());
    set_secret(&f.rt, INSTANCE, "api_token", "tok-123").unwrap();
    assert_eq!(f.secrets.get(&format!("connector-{INSTANCE}-api_token")).unwrap().as_deref(), Some("tok-123"));
    set_secret(&f.rt, INSTANCE, "refresh_token", "tok-456").unwrap();
    let err = set_secret(&f.rt, INSTANCE, "altro", "tok").unwrap_err();
    assert!(err.contains("non previsto"), "{err}");
    let bad = set_secret(&f.rt, INSTANCE, "api_token", "con spazi SEGRETO").unwrap_err();
    assert!(!bad.contains("SEGRETO"), "{bad}");
    assert_eq!(f.secrets.get(&format!("connector-{INSTANCE}-api_token")).unwrap().as_deref(), Some("tok-123"));
    clear_secret(&f.rt, INSTANCE, "api_token").unwrap();
    assert_eq!(f.secrets.get(&format!("connector-{INSTANCE}-api_token")).unwrap(), None);
    assert!(clear_secret(&f.rt, INSTANCE, "altro").is_err());
    assert!(set_secret(&f.rt, "con-9-9", "api_token", "tok").is_err());
}

#[test]
fn a_new_secret_restarts_an_instance_stopped_for_auth() {
    let f = fixture(contact_file());
    f.rt.set_hub_enabled(true);
    wait_for("avviata", || status(&f.rt, INSTANCE).is_some_and(|s| s != HealthState::Disabled));
    // Il task e' fermo per credenziali rifiutate (stato scritto dall'epoca corrente).
    let epoch = f.rt.status_registry().begin_run(INSTANCE);
    f.rt.status_registry().run_failed(INSTANCE, epoch, HealthState::NeedsAuth, "x", None);
    set_secret(&f.rt, INSTANCE, "bot_token", "tok-nuovo").unwrap();
    wait_for("riavviata", || status(&f.rt, INSTANCE) != Some(HealthState::NeedsAuth));
    f.rt.stop_all();
}

#[test]
fn a_secret_does_not_restart_a_healthy_instance() {
    let f = fixture(contact_file());
    f.rt.set_hub_enabled(true);
    wait_for("ok", || status(&f.rt, INSTANCE) == Some(HealthState::Ok));
    let epoch = f.rt.status_registry().begin_run(INSTANCE);
    f.rt.status_registry().run_ok(INSTANCE, epoch, 77);
    set_secret(&f.rt, INSTANCE, "bot_token", "tok-nuovo").unwrap();
    // Un riavvio avrebbe aperto una nuova esecuzione, invalidando la nostra epoca.
    f.rt.status_registry().run_ok(INSTANCE, epoch, 78);
    assert_eq!(f.rt.status_registry().get(INSTANCE).unwrap().last_ok_at, Some(78), "nessun riavvio");
    f.rt.stop_all();
}

#[test]
fn the_view_shows_which_slots_are_set_but_never_their_values() {
    let f = fixture(source_file());
    set_secret(&f.rt, INSTANCE, "api_token", "tok-SEGRETO").unwrap();
    let instance = f.rt.store().instance(INSTANCE).unwrap();
    let view = build_view(&f.rt, instance);
    let slots: Vec<(String, bool)> = view.secrets.iter().map(|s| (s.slot.clone(), s.set)).collect();
    assert_eq!(slots, vec![("api_token".to_string(), true), ("refresh_token".to_string(), false)]);
    assert_eq!(view.status.state, HealthState::Disabled);
    let json = serde_json::to_string(&view).unwrap();
    assert!(!json.contains("SEGRETO"), "{json}");
}
