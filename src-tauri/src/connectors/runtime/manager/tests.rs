use super::*;
use crate::connectors::contracts::ConnectorError;
use crate::connectors::model::{ConnectorsFile, LinkThread, RouteThread, RouteWake};
use crate::connectors::registry::test_registry::Registered;
use crate::connectors::runtime::health::ConnectorStatus;
use crate::connectors::testing::{
    contact_notice, instance, link, new_contact, new_source, route, FakeClock, FakeHub, FakeSource, MemoryStates,
    ParkedSleeper, RecordingEmitter, INSTANCE,
};
use crate::hub::{Channel, NoticeSink};
use crate::secrets::MemorySecrets;
use serde_json::json;
use std::time::{Duration, Instant};

struct Fixture {
    rt: ConnectorsRuntime,
    emitter: Arc<RecordingEmitter>,
    source: &'static FakeSource,
    _registered: Registered,
}

fn fixture(file: ConnectorsFile) -> Fixture {
    let source = new_source();
    let registered = Registered::new(vec![ConnectorImpl::Source(source), ConnectorImpl::Contact(new_contact())]);
    let rt = ConnectorsRuntime::default();
    let emitter = Arc::new(RecordingEmitter::default());
    rt.statuses.set_emitter(emitter.clone());
    rt.store.replace(file);
    rt.install(Platform {
        hub: Arc::new(FakeHub::default()),
        clock: Arc::new(FakeClock::new(1_700_000_000_000)),
        sleeper: Arc::new(ParkedSleeper),
        states: Arc::new(MemoryStates::default()),
        secrets: Arc::new(MemorySecrets::default()),
    });
    Fixture { rt, emitter, source, _registered: registered }
}

fn source_file() -> ConnectorsFile {
    ConnectorsFile {
        instances: vec![instance(INSTANCE, "fake-source", json!({}), 2)],
        routes: vec![route("rte-1-1", RouteThread::NewPerItem, RouteWake::None)],
        links: Vec::new(),
    }
}

fn contact_file() -> ConnectorsFile {
    ConnectorsFile {
        instances: vec![instance(INSTANCE, "fake-contact", json!({}), 1)],
        routes: Vec::new(),
        links: Vec::new(),
    }
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(5), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn state_of(rt: &ConnectorsRuntime, id: &str) -> Option<HealthState> {
    rt.statuses().into_iter().find(|s: &ConnectorStatus| s.instance == id).map(|s| s.state)
}

fn env_refs(rt: &ConnectorsRuntime) -> usize {
    Arc::strong_count(rt.env.get().unwrap())
}

#[test]
fn with_the_hub_off_nothing_runs_and_everything_is_disabled() {
    let f = fixture(source_file());
    f.rt.reconcile();
    assert!(f.rt.running_ids().is_empty());
    assert_eq!(state_of(&f.rt, INSTANCE), Some(HealthState::Disabled));
}

#[test]
fn an_enabled_instance_starts_with_the_hub_on_and_reports_ok() {
    let f = fixture(source_file());
    f.rt.set_hub_enabled(true);
    assert_eq!(f.rt.running_ids(), vec![INSTANCE.to_string()]);
    wait_for("stato ok", || state_of(&f.rt, INSTANCE) == Some(HealthState::Ok));
    f.rt.stop_all();
}

#[test]
fn turning_the_hub_off_aborts_the_tasks_and_marks_everything_disabled() {
    let f = fixture(source_file());
    f.rt.set_hub_enabled(true);
    wait_for("stato ok", || state_of(&f.rt, INSTANCE) == Some(HealthState::Ok));
    assert!(env_refs(&f.rt) > 1, "il task tiene l'ambiente");
    f.rt.set_hub_enabled(false);
    assert!(f.rt.running_ids().is_empty());
    assert_eq!(state_of(&f.rt, INSTANCE), Some(HealthState::Disabled));
    wait_for("task interrotto", || env_refs(&f.rt) == 1);
    let last = f.emitter.0.lock().unwrap().last().cloned().unwrap();
    assert!(last.iter().all(|s| s.state == HealthState::Disabled), "l'ultimo elenco emesso e' tutto disabilitato");
}

#[test]
fn disabling_an_instance_stops_it_and_removing_it_drops_its_status() {
    let f = fixture(source_file());
    f.rt.set_hub_enabled(true);
    wait_for("stato ok", || state_of(&f.rt, INSTANCE) == Some(HealthState::Ok));
    f.rt.mutate(|file| {
        file.instance_mut(INSTANCE).unwrap().enabled = false;
        Ok(())
    })
    .unwrap();
    assert!(f.rt.running_ids().is_empty());
    assert_eq!(state_of(&f.rt, INSTANCE), Some(HealthState::Disabled));
    wait_for("task interrotto", || env_refs(&f.rt) == 1);
    f.rt.mutate(|file| {
        file.remove_instance(INSTANCE);
        Ok(())
    })
    .unwrap();
    assert_eq!(state_of(&f.rt, INSTANCE), None);
}

#[test]
fn an_unknown_connector_is_unavailable_and_kept_untouched() {
    let file = ConnectorsFile {
        instances: vec![instance(INSTANCE, "connettore-sparito", json!({"x": 1}), 7)],
        ..ConnectorsFile::default()
    };
    let f = fixture(file.clone());
    f.rt.set_hub_enabled(true);
    assert_eq!(state_of(&f.rt, INSTANCE), Some(HealthState::Unavailable));
    assert!(f.rt.running_ids().is_empty());
    assert_eq!(f.rt.snapshot(), file, "ne' migrata ne' scartata");
}

#[test]
fn an_old_config_is_migrated_in_memory_and_a_failed_migration_keeps_the_original() {
    let mut file = source_file();
    file.instances[0].config_version = 1;
    let f = fixture(file);
    let migrated = f.rt.snapshot();
    assert_eq!((migrated.instances[0].config_version, &migrated.instances[0].config["interval_secs"]), (2, &json!(60)));
    let broken = fixture(ConnectorsFile {
        instances: vec![instance(INSTANCE, "fake-contact", json!({"vecchio": true}), 0)],
        ..ConnectorsFile::default()
    });
    broken.rt.set_hub_enabled(true);
    assert_eq!(state_of(&broken.rt, INSTANCE), Some(HealthState::Error));
    assert!(broken.rt.running_ids().is_empty());
    let kept = broken.rt.snapshot();
    assert_eq!((kept.instances[0].config_version, &kept.instances[0].config), (0, &json!({"vecchio": true})));
}

#[test]
fn an_invalid_config_goes_to_error_without_starting() {
    let mut file = source_file();
    file.instances[0].config = json!({"mailbox": "vietata"});
    let f = fixture(file);
    f.rt.set_hub_enabled(true);
    let status = f.rt.statuses().remove(0);
    assert_eq!((status.state, status.last_error.as_deref()), (HealthState::Error, Some("Configurazione non valida.")));
    assert!(f.rt.running_ids().is_empty());
}

#[test]
fn a_task_stopped_by_an_auth_error_restarts_only_on_request() {
    let f = fixture(source_file());
    f.source.push(Err(ConnectorError::Auth("x".to_string())));
    f.rt.set_hub_enabled(true);
    wait_for("needs_auth", || state_of(&f.rt, INSTANCE) == Some(HealthState::NeedsAuth));
    f.rt.reconcile();
    assert_eq!(f.source.polls(), 1, "reconcile non lo riavvia");
    f.rt.restart_instance(INSTANCE);
    wait_for("di nuovo ok", || state_of(&f.rt, INSTANCE) == Some(HealthState::Ok));
    assert_eq!(f.source.polls(), 2);
    f.rt.stop_all();
}

#[test]
fn a_contact_instance_registers_for_notices_and_stopping_drops_them() {
    let f = fixture(contact_file());
    f.rt.set_hub_enabled(true);
    assert_eq!(f.rt.notices.registered(), 1);
    let notice = contact_notice("msg-1", "testo");
    f.rt.notice_sink().deliver(&notice);
    f.rt.set_hub_enabled(false);
    assert_eq!(f.rt.notices.registered(), 0);
    f.rt.notice_sink().deliver(&notice);
    // Una fonte non si registra mai fra i destinatari.
    let source = fixture(source_file());
    source.rt.set_hub_enabled(true);
    assert_eq!(source.rt.notices.registered(), 0);
    source.rt.stop_all();
}

#[test]
fn a_removed_workspace_takes_its_routes_and_current_threads_with_it() {
    let ws = Channel::Workspace("ws-1".to_string());
    let mut in_ws = route("rte-1-2", RouteThread::PerConversation, RouteWake::None);
    in_ws.channel = ws.clone();
    let mut file = source_file();
    file.routes.push(in_ws);
    let mut on_ws = link("lnk-1-1", "u1", None);
    on_ws.current = Some(LinkThread { channel: ws.clone(), thread_id: "thr-1".to_string() });
    file.links = vec![on_ws, link("lnk-1-2", "u2", Some("thr-2"))];
    let f = fixture(file);
    f.rt.forget_channel(&ws).unwrap();
    let after = f.rt.snapshot();
    assert_eq!(after.routes.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["rte-1-1"]);
    assert!(after.links[0].current.is_none() && after.links[1].current.is_some());
    // Senza nulla da togliere non si riscrive niente.
    f.rt.forget_channel(&Channel::Workspace("ws-9".to_string())).unwrap();
    assert_eq!(f.rt.snapshot(), after);
}

#[test]
fn a_context_is_built_for_known_instances_only() {
    let f = fixture(source_file());
    let (ctx, imp) = tauri::async_runtime::block_on(f.rt.context_for(INSTANCE)).unwrap();
    assert_eq!((ctx.instance(), imp.descriptor().id), (INSTANCE, "fake-source"));
    assert!(tauri::async_runtime::block_on(f.rt.context_for("con-9-9")).is_err());
}

#[test]
fn pairing_codes_come_from_the_runtime_clock() {
    let f = fixture(contact_file());
    let code = f.rt.issue_pairing_code(INSTANCE).unwrap();
    assert_eq!(code.len(), 12);
    assert!(ConnectorsRuntime::default().issue_pairing_code(INSTANCE).is_err(), "non installato");
}
