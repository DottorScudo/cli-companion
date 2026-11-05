use super::*;
use crate::connectors::contracts::ConnectorError;
use std::sync::Mutex;

#[derive(Default)]
pub struct Recorder(pub Mutex<Vec<Vec<ConnectorStatus>>>);

impl StatusEmitter for Recorder {
    fn emit(&self, statuses: &[ConnectorStatus]) {
        self.0.lock().unwrap().push(statuses.to_vec());
    }
}

fn registry() -> (StatusRegistry, Arc<Recorder>) {
    let registry = StatusRegistry::default();
    let recorder = Arc::new(Recorder::default());
    registry.set_emitter(recorder.clone());
    (registry, recorder)
}

#[test]
fn every_change_emits_the_full_list_and_an_identical_state_does_not() {
    let (registry, recorder) = registry();
    registry.set_state("con-1-1", HealthState::Starting);
    registry.set_state("con-1-1", HealthState::Starting);
    registry.mark_ok("con-1-2", 50);
    let emitted = recorder.0.lock().unwrap();
    assert_eq!(emitted.len(), 2);
    assert_eq!(emitted[1].len(), 2, "l'elenco intero, non solo l'istanza cambiata");
    assert_eq!(emitted[1][1].last_ok_at, Some(50));
}

#[test]
fn a_failure_keeps_the_last_success_and_a_success_clears_the_error() {
    let (registry, _) = registry();
    registry.mark_ok("con-1-1", 10);
    registry.mark_failed("con-1-1", HealthState::Backoff, "x", Some(99));
    let s = registry.get("con-1-1").unwrap();
    assert_eq!((s.state, s.last_ok_at, s.next_retry_at), (HealthState::Backoff, Some(10), Some(99)));
    registry.mark_ok("con-1-1", 20);
    let s = registry.get("con-1-1").unwrap();
    assert_eq!((s.state, s.last_error, s.next_retry_at), (HealthState::Ok, None, None));
}

#[test]
fn removed_instances_disappear_and_the_list_is_announced() {
    let (registry, recorder) = registry();
    registry.set_state("con-1-1", HealthState::Disabled);
    registry.set_state("con-1-2", HealthState::Disabled);
    registry.retain(&HashSet::from(["con-1-2".to_string()]));
    registry.remove("con-1-9");
    assert_eq!(registry.list().len(), 1);
    assert_eq!(recorder.0.lock().unwrap().len(), 3);
    registry.remove("con-1-2");
    assert!(registry.list().is_empty());
}

#[test]
fn the_error_text_depends_only_on_the_class_never_on_the_service_text() {
    let secret = "sk-SEGRETISSIMO";
    let errors = [
        ConnectorError::Transient(format!("GET https://api.example.com/bot{secret}/getUpdates: boom")),
        ConnectorError::Auth(secret.to_string()),
        ConnectorError::Config(secret.to_string()),
        ConnectorError::Fatal(secret.to_string()),
        ConnectorError::RateLimited { retry_after: None },
    ];
    let (registry, _) = registry();
    for (i, error) in errors.iter().enumerate() {
        registry.mark_failed(&format!("con-1-{i}"), HealthState::Error, error.public_message(), None);
    }
    let json = serde_json::to_string(&registry.list()).unwrap();
    assert!(!json.contains(secret) && !json.contains("https://") && !json.contains("bot"), "{json}");
    assert!(json.contains("Credenziali rifiutate"));
}

#[test]
fn the_wire_format_uses_snake_case_states() {
    let status = ConnectorStatus {
        instance: "con-1-1".to_string(),
        state: HealthState::NeedsAuth,
        last_ok_at: None,
        last_error: None,
        next_retry_at: None,
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["state"], "needs_auth");
    assert_eq!(serde_json::to_value(HealthState::Unavailable).unwrap(), "unavailable");
}

#[test]
fn only_the_current_run_can_write_the_status() {
    let (registry, _) = registry();
    let first = registry.begin_run("con-1-1");
    registry.run_state("con-1-1", first, HealthState::Starting);
    registry.run_ok("con-1-1", first, 10);
    // Il manager ferma l'istanza (HUB spento): lo stato e' deciso da lui.
    registry.end_run("con-1-1");
    registry.set_state("con-1-1", HealthState::Disabled);
    // Un "ok" tardivo del task interrotto non lo sovrascrive.
    registry.run_ok("con-1-1", first, 20);
    registry.run_failed("con-1-1", first, HealthState::Backoff, "x", Some(1));
    registry.run_state("con-1-1", first, HealthState::Starting);
    let s = registry.get("con-1-1").unwrap();
    assert_eq!((s.state, s.last_ok_at, s.last_error), (HealthState::Disabled, Some(10), None));
    // Una nuova esecuzione scrive; la vecchia epoca resta esclusa.
    let second = registry.begin_run("con-1-1");
    assert_ne!(first, second);
    registry.run_ok("con-1-1", first, 30);
    assert_eq!(registry.get("con-1-1").unwrap().last_ok_at, Some(10));
    registry.run_ok("con-1-1", second, 40);
    assert_eq!(registry.get("con-1-1").unwrap().state, HealthState::Ok);
    registry.remove("con-1-1");
    registry.run_ok("con-1-1", second, 50);
    assert!(registry.get("con-1-1").is_none(), "un'istanza rimossa non riappare");
}
