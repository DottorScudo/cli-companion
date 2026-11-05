use super::*;
use crate::connectors::contracts::{ConnectorImpl, ReceiveBatch};
use crate::connectors::descriptor::DescriptorLimits;
use crate::connectors::model::{ConnectorsFile, RouteThread, RouteWake, THREAD_GONE};
use crate::connectors::runtime::health::ConnectorStatus;
use crate::connectors::state::MAIN_CURSOR;
use crate::connectors::testing::{
    batch, inbound, instance, item, link, new_contact, new_source, route, FakeClock, FakeContact, FakeHub, FakeSource,
    HubCall, InstantSleeper, MemoryStates, RecordingEmitter, FAKE_CONTACT, FAKE_SOURCE, INSTANCE,
};
use crate::hub::{Channel, HubNoticeSound};
use crate::secrets::MemorySecrets;
use serde_json::json;
use std::time::Duration;

struct Rig {
    env: Arc<Env>,
    hub: Arc<FakeHub>,
    states: Arc<MemoryStates>,
    sleeper: Arc<InstantSleeper>,
    secrets: Arc<MemorySecrets>,
    emitter: Arc<RecordingEmitter>,
    clock: Arc<FakeClock>,
}

impl Rig {
    /// `budget`: attese che il ciclo accetta prima di fermarsi.
    fn new(budget: usize, file: ConnectorsFile) -> Self {
        let hub = Arc::new(FakeHub::default());
        let states = Arc::new(MemoryStates::default());
        let sleeper = Arc::new(InstantSleeper::with_budget(budget));
        let secrets = Arc::new(MemorySecrets::default());
        let clock = Arc::new(FakeClock::new(1_700_000_000_000));
        let emitter = Arc::new(RecordingEmitter::default());
        let statuses = Arc::new(StatusRegistry::default());
        statuses.set_emitter(emitter.clone());
        let store = Arc::new(ConnectorsStore::default());
        store.replace(file);
        let env = Env {
            hub: hub.clone(),
            clock: clock.clone(),
            sleeper: sleeper.clone(),
            store,
            states: states.clone(),
            statuses,
            secrets: secrets.clone(),
            client: OnceLock::new(),
            pairing: Arc::new(PairingRegistry::default()),
            ids: IdGenerator::default(),
        };
        Self { env: Arc::new(env), hub, states, sleeper, secrets, emitter, clock }
    }

    fn source(budget: usize, routes: Vec<crate::connectors::model::ConnectorRoute>) -> Self {
        let file = ConnectorsFile {
            instances: vec![instance(INSTANCE, "fake-source", json!({}), 2)],
            routes,
            links: Vec::new(),
        };
        Self::new(budget, file)
    }

    fn run_source(&self, source: &'static FakeSource, config: serde_json::Value) {
        let inst = instance(INSTANCE, "fake-source", config, 2);
        let epoch = self.env.statuses.begin_run(INSTANCE);
        let job = Job {
            instance: inst,
            imp: ConnectorImpl::Source(source),
            state: StateHandle::default(),
            notices: None,
            epoch,
        };
        tauri::async_runtime::block_on(run(self.env.clone(), job));
    }

    fn run_contact(&self, contact: &'static FakeContact) {
        let job = Job {
            instance: instance(INSTANCE, "fake-contact", json!({}), 1),
            imp: ConnectorImpl::Contact(contact),
            state: StateHandle::default(),
            notices: None,
            epoch: self.env.statuses.begin_run(INSTANCE),
        };
        tauri::async_runtime::block_on(run(self.env.clone(), job));
    }

    fn status(&self) -> ConnectorStatus {
        self.env.statuses.get(INSTANCE).expect("stato presente")
    }

    fn history(&self) -> Vec<HealthState> {
        self.emitter.0.lock().unwrap().iter().filter_map(|l| l.first().map(|s| s.state)).collect()
    }
}

fn per_item(id: &str) -> crate::connectors::model::ConnectorRoute {
    route(id, RouteThread::NewPerItem, RouteWake::None)
}

fn transient() -> Result<crate::connectors::contracts::PollBatch, ConnectorError> {
    Err(ConnectorError::Transient("giu".to_string()))
}

fn empty() -> Result<crate::connectors::contracts::PollBatch, ConnectorError> {
    Ok(batch(Vec::new(), json!(1), false))
}

fn within(wait: Duration, base_secs: f64) -> bool {
    let secs = wait.as_secs_f64();
    secs >= base_secs * 0.75 - 1e-9 && secs <= base_secs + 1e-9
}

#[test]
fn transient_errors_back_off_exponentially_and_a_success_resets_the_sequence() {
    let rig = Rig::source(4, vec![per_item("rte-1-1")]);
    let source = new_source();
    for result in [transient(), transient(), empty(), transient()] {
        source.push(result);
    }
    rig.run_source(source, json!({}));
    let sleeps = rig.sleeper.sleeps.lock().unwrap().clone();
    assert_eq!(sleeps.len(), 5, "{sleeps:?}");
    assert!(within(sleeps[0], 5.0) && within(sleeps[1], 10.0), "{sleeps:?}");
    assert_eq!(sleeps[2], Duration::from_secs(60), "dopo il successo si aspetta l'intervallo");
    assert!(within(sleeps[3], 5.0), "il backoff riparte da capo: {sleeps:?}");
    assert_eq!(sleeps[4], Duration::from_secs(60));
    let history = rig.history();
    assert_eq!(history[0], HealthState::Starting);
    assert!(history.contains(&HealthState::Backoff) && history.last() == Some(&HealthState::Ok));
    let status = rig.status();
    assert_eq!((status.state, status.last_ok_at), (HealthState::Ok, Some(rig.clock.now_ms())));
}

#[test]
fn the_backoff_status_carries_a_generic_message_and_the_retry_time() {
    let rig = Rig::source(1, vec![per_item("rte-1-1")]);
    let source = new_source();
    source.push(transient());
    rig.run_source(source, json!({}));
    let backoff = rig
        .emitter
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|l| l.first().cloned())
        .find(|s| s.state == HealthState::Backoff);
    let backoff = backoff.expect("uno stato backoff e' stato emesso");
    assert_eq!(backoff.last_error.as_deref(), Some("Servizio non raggiungibile: nuovo tentativo a breve."));
    assert!(backoff.next_retry_at.unwrap() > rig.clock.now_ms());
}

#[test]
fn rate_limited_honours_retry_after_never_below_the_backoff_and_capped_at_an_hour() {
    let rig = Rig::source(2, vec![per_item("rte-1-1")]);
    let source = new_source();
    source.push(Err(ConnectorError::RateLimited { retry_after: Some(Duration::from_secs(90)) }));
    source.push(Err(ConnectorError::RateLimited { retry_after: Some(Duration::from_secs(999_999)) }));
    rig.run_source(source, json!({}));
    assert_eq!(rig.sleeper.secs()[..2], [90, 3600]);
    assert_eq!(rig.history().iter().filter(|s| **s == HealthState::Backoff).count(), 2);
}

#[test]
fn an_auth_error_ends_the_loop_in_needs_auth() {
    let rig = Rig::source(10, vec![per_item("rte-1-1")]);
    let source = new_source();
    source.push(Err(ConnectorError::Auth("scaduto".to_string())));
    rig.run_source(source, json!({}));
    let status = rig.status();
    assert_eq!(status.state, HealthState::NeedsAuth);
    assert_eq!(status.last_error.as_deref(), Some("Credenziali rifiutate: serve il tuo intervento."));
    assert_eq!(status.next_retry_at, None);
    assert_eq!((source.polls(), rig.sleeper.secs()), (1, Vec::<u64>::new()), "ritorna senza riprovare");
}

#[test]
fn config_and_fatal_errors_end_the_loop_in_error() {
    for error in [ConnectorError::Config("x".to_string()), ConnectorError::Fatal("x".to_string())] {
        let rig = Rig::source(10, vec![per_item("rte-1-1")]);
        let source = new_source();
        source.push(Err(error));
        rig.run_source(source, json!({}));
        assert_eq!(rig.status().state, HealthState::Error);
        assert_eq!((source.polls(), rig.sleeper.secs().len()), (1, 0));
    }
}

#[test]
fn statuses_never_contain_secrets_urls_or_service_text() {
    let rig = Rig::source(1, vec![per_item("rte-1-1")]);
    let secret = "sk-SEGRETISSIMO-123";
    rig.secrets.set(&format!("connector-{INSTANCE}-api_token"), secret).unwrap();
    let source = new_source();
    source.push(Err(ConnectorError::Transient(format!("GET https://api.fake-source.example/{secret}: 500 boom"))));
    source.push(Err(ConnectorError::Auth(format!("Bearer {secret}"))));
    rig.run_source(source, json!({}));
    let everything =
        serde_json::to_string(&rig.emitter.0.lock().unwrap().clone().iter().flatten().collect::<Vec<_>>()).unwrap();
    for needle in [secret, "https://", "boom", "Bearer"] {
        assert!(!everything.contains(needle), "{needle}: {everything}");
    }
    assert!(everything.contains("needs_auth"));
}

#[test]
fn a_round_saves_the_state_and_waits_the_configured_interval() {
    let rig = Rig::source(0, vec![per_item("rte-1-1")]);
    let source = new_source();
    source.push(Ok(batch(vec![item("rte-1-1", "a", None)], json!(5), false)));
    rig.run_source(source, json!({"interval_secs": 300}));
    assert_eq!(rig.sleeper.secs(), vec![300]);
    let state = rig.states.get(INSTANCE);
    assert_eq!(state.cursor(MAIN_CURSOR), Some(&json!(5)));
    assert!(state.is_seen("rte-1-1", "a") && rig.states.saves() >= 1);
    assert_eq!(rig.hub.calls().len(), 1);
}

#[test]
fn a_loaded_state_is_used_and_a_delivered_item_is_not_repeated_after_a_restart() {
    let rig = Rig::source(0, vec![per_item("rte-1-1")]);
    let mut saved = crate::connectors::state::InstanceState::default();
    saved.mark_seen("rte-1-1", "a");
    saved.set_cursor(MAIN_CURSOR, json!(4));
    rig.states.map.lock().unwrap().insert(INSTANCE.to_string(), saved);
    let source = new_source();
    source.push(Ok(batch(vec![item("rte-1-1", "a", None)], json!(5), false)));
    rig.run_source(source, json!({}));
    assert!(rig.hub.calls().is_empty());
}

#[test]
fn the_interval_never_goes_below_a_minute_or_the_descriptor_minimum() {
    assert_eq!(source_interval(&FAKE_SOURCE, &json!({})), Duration::from_secs(60));
    assert_eq!(source_interval(&FAKE_SOURCE, &json!({"interval_secs": 10})), Duration::from_secs(60));
    assert_eq!(source_interval(&FAKE_SOURCE, &json!({"interval_secs": 900})), Duration::from_secs(900));
    assert_eq!(source_interval(&FAKE_SOURCE, &json!({"interval_secs": "x"})), Duration::from_secs(60));
    // Un descrittore senza il campo ignora la configurazione; uno con un minimo piu' stretto lo rispetta.
    assert_eq!(source_interval(&FAKE_CONTACT, &json!({"interval_secs": 900})), Duration::from_secs(60));
    static STRICT: ConnectorDescriptor = ConnectorDescriptor {
        limits: DescriptorLimits { items_per_round: Some(5), min_interval_secs: Some(300) },
        ..FAKE_SOURCE
    };
    assert_eq!(source_interval(&STRICT, &json!({"interval_secs": 100})), Duration::from_secs(300));
    assert_eq!((items_per_round(&STRICT), items_per_round(&FAKE_SOURCE)), (5, ITEMS_PER_ROUND));
}

#[test]
fn more_items_ready_means_the_next_round_starts_without_waiting() {
    let rig = Rig::source(0, vec![per_item("rte-1-1")]);
    let source = new_source();
    source.push(Ok(batch(vec![item("rte-1-1", "a", None)], json!(1), true)));
    source.push(Ok(batch(vec![item("rte-1-1", "b", None)], json!(2), false)));
    rig.run_source(source, json!({}));
    assert_eq!((source.polls(), rig.sleeper.secs()), (2, vec![60]));
    assert_eq!(rig.hub.calls().len(), 2);
}

#[test]
fn a_vanished_thread_disables_the_route_in_the_file_and_the_other_route_still_delivers() {
    let gone = route("rte-1-1", RouteThread::Existing { thread_id: "thr-sparito".to_string() }, RouteWake::None);
    let rig = Rig::source(0, vec![gone, per_item("rte-1-2")]);
    let source = new_source();
    let items = vec![item("rte-1-1", "x", None), item("rte-1-2", "y", None)];
    source.push(Ok(batch(items.clone(), json!(5), false)));
    source.push(Ok(batch(items, json!(5), false)));
    rig.run_source(source, json!({}));
    let file = rig.env.store.snapshot();
    assert!(!file.routes[0].enabled && file.routes[0].error.as_deref() == Some(THREAD_GONE));
    assert!(file.routes[1].enabled);
    let delivered: Vec<String> = rig
        .hub
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            HubCall::Deliver(d) => Some(d.origin.external_id),
            _ => None,
        })
        .collect();
    assert_eq!(delivered, vec!["y"]);
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), Some(&json!(5)));
}

fn contact_rig(budget: usize) -> Rig {
    let file = ConnectorsFile {
        instances: vec![instance(INSTANCE, "fake-contact", json!({}), 1)],
        routes: Vec::new(),
        links: vec![link("lnk-1-1", "u1", Some("thr-1"))],
    };
    let rig = Rig::new(budget, file);
    rig.hub.add_thread(&Channel::General, "thr-1");
    rig
}

#[test]
fn the_contact_loop_forwards_inbound_messages_then_saves_the_cursor() {
    let rig = contact_rig(0);
    let contact = new_contact();
    let messages = vec![inbound("m1", "u1", "ciao"), inbound("m2", "sconosciuto", "x")];
    contact.push(Ok(ReceiveBatch { messages, cursor: json!(10) }));
    rig.run_contact(contact);
    let calls = rig.hub.calls();
    assert!(matches!(calls.as_slice(), [HubCall::SendUser(m)] if m.body == "ciao" && m.origin.is_some()), "{calls:?}");
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), Some(&json!(10)));
    assert_eq!(rig.sleeper.secs(), vec![1], "pausa solo dopo una ricezione vuota");
    assert_eq!(rig.status().state, HealthState::Ok);
}

#[test]
fn a_hub_failure_in_a_batch_keeps_the_old_cursor_and_backs_off() {
    let rig = contact_rig(0);
    let contact = new_contact();
    *rig.hub.fail_with.lock().unwrap() = Some("HUB spento".to_string());
    contact.push(Ok(ReceiveBatch { messages: vec![inbound("m1", "u1", "ciao")], cursor: json!(10) }));
    rig.run_contact(contact);
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), None);
    assert!(rig.history().contains(&HealthState::Backoff));
}

#[test]
fn a_contact_auth_error_ends_the_loop() {
    let rig = contact_rig(10);
    let contact = new_contact();
    contact.push(Err(ConnectorError::Auth("x".to_string())));
    rig.run_contact(contact);
    assert_eq!(rig.status().state, HealthState::NeedsAuth);
    assert!(rig.sleeper.secs().is_empty());
}

fn notice(n: usize) -> HubNotice {
    HubNotice {
        channel: Channel::General,
        thread_id: "thr-1".to_string(),
        message_id: format!("msg-{n}"),
        title: format!("Titolo {n}"),
        body: "corpo".to_string(),
        sound: HubNoticeSound::Ding,
    }
}

fn run_notices_for(rig: &Rig, contact: &'static FakeContact, count: usize) -> bool {
    let state = StateHandle::default();
    let t = crate::connectors::testing::test_context(&FAKE_CONTACT, json!({}));
    let cc = ContactCtx {
        connector: contact,
        ctx: &t.ctx,
        hub: &*rig.hub,
        state: &state,
        store: &rig.env.store,
        pairing: &rig.env.pairing,
        ids: &rig.env.ids,
        clock: &*rig.clock,
    };
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    for n in 0..count {
        tx.try_send(notice(n)).unwrap();
    }
    drop(tx);
    tauri::async_runtime::block_on(crate::connectors::runtime::contact_loop::run_notices(
        &rig.env,
        INSTANCE,
        rig.env.statuses.begin_run(INSTANCE),
        &cc,
        rx,
    ))
}

#[test]
fn queued_notices_are_sent_to_the_links_and_the_state_is_saved() {
    let rig = contact_rig(0);
    let contact = new_contact();
    assert!(run_notices_for(&rig, contact, 2), "il canale si e' chiuso");
    let texts: Vec<String> = contact.sent.lock().unwrap().iter().map(|m| m.text.clone()).collect();
    assert_eq!(texts, vec!["Titolo 0\ncorpo", "Titolo 1\ncorpo"]);
    assert_eq!(rig.states.saves(), 2);
}

#[test]
fn an_auth_error_while_sending_a_notice_stops_the_supervisor() {
    let rig = contact_rig(0);
    let contact = new_contact();
    *contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    assert!(!run_notices_for(&rig, contact, 1));
    assert_eq!(rig.status().state, HealthState::NeedsAuth);
    // Un errore transitorio invece si registra e basta.
    let rig = contact_rig(0);
    let contact = new_contact();
    *contact.send_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    assert!(run_notices_for(&rig, contact, 1));
}
