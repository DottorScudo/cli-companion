use super::*;
use crate::connectors::commands::support::{contact_file, fixture, source_file};
use crate::connectors::runtime::health::HealthState;
use crate::connectors::testing::INSTANCE;
use std::time::{Duration, Instant};

fn run(rt: &ConnectorsRuntime, id: &str, action: &str) -> Result<ActionResult, String> {
    tauri::async_runtime::block_on(execute(rt, id, action))
}

fn wait_running(rt: &ConnectorsRuntime) {
    let start = Instant::now();
    while rt.status_registry().get(INSTANCE).is_none_or(|s| s.state == HealthState::Disabled) {
        assert!(start.elapsed() < Duration::from_secs(5), "timeout: avvio");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn test_runs_the_declared_action_even_on_a_disabled_instance() {
    let f = fixture(source_file());
    let out = run(&f.rt, INSTANCE, "test").unwrap();
    assert_eq!(out.message, "Connessione riuscita.");
    assert_eq!(out.pairing_code, None);
}

#[test]
fn only_declared_actions_run() {
    let f = fixture(source_file());
    assert!(run(&f.rt, INSTANCE, "pair").is_err(), "la fonte non dichiara pair");
    assert!(run(&f.rt, INSTANCE, "authorize").is_err());
    assert!(run(&f.rt, INSTANCE, "boom").is_err());
    assert!(run(&f.rt, "con-9-9", "test").is_err());
    assert!(run(&f.rt, "non valido", "test").is_err());
}

#[test]
fn pair_issues_a_one_time_code_reachable_by_the_connector() {
    let mut file = contact_file();
    file.instances[0].enabled = true;
    let f = fixture(file);
    let refused = run(&f.rt, INSTANCE, "pair").unwrap_err();
    assert!(refused.contains("Attiva"), "senza HUB l'istanza non gira: {refused}");
    f.rt.set_hub_enabled(true);
    wait_running(&f.rt);
    let out = run(&f.rt, INSTANCE, "pair").unwrap();
    let code = out.pairing_code.expect("codice");
    assert_eq!(code.len(), crate::connectors::runtime::pairing::CODE_LEN);
    assert_eq!(out.detail, Some(format!("invito:{code}")), "il connettore vede lo stesso codice");
    let other = run(&f.rt, INSTANCE, "pair").unwrap().pairing_code.unwrap();
    assert_ne!(code, other);
    f.rt.stop_all();
}

#[test]
fn other_actions_get_no_pairing_code() {
    let f = fixture(contact_file());
    let out = run(&f.rt, INSTANCE, "test").unwrap();
    assert_eq!((out.detail.as_deref(), out.pairing_code), (Some("dettaglio"), None));
}
