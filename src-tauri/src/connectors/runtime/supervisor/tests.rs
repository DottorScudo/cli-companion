use super::*;
use crate::connectors::contracts::{ConnectorImpl, ReceiveBatch};
use crate::connectors::descriptor::DescriptorLimits;
use crate::connectors::model::{ConnectorsFile, RouteThread, RouteWake, THREAD_GONE};
use crate::connectors::runtime::health::ConnectorStatus;
use crate::connectors::state::{conversation_key, MAIN_CURSOR};
use crate::connectors::testing::{
    batch, contact_notice, inbound, instance, item, link, new_contact, new_source, route, FakeClock, FakeContact,
    FakeHub, FakeSource, HubCall, InstantSleeper, MemoryStates, RecordingEmitter, FAKE_CONTACT, FAKE_SOURCE, INSTANCE,
};
use crate::hub::{Channel, ContactNotice};
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
        limits: DescriptorLimits {
            items_per_round: Some(5),
            min_interval_secs: Some(300),
            default_interval_secs: None,
        },
        ..FAKE_SOURCE
    };
    assert_eq!(source_interval(&STRICT, &json!({"interval_secs": 100})), Duration::from_secs(300));
    assert_eq!((items_per_round(&STRICT), items_per_round(&FAKE_SOURCE)), (5, ITEMS_PER_ROUND));
}

#[test]
fn the_interval_can_be_given_in_minutes_or_default_to_the_descriptor() {
    const FIELD: crate::connectors::descriptor::ConfigField = crate::connectors::descriptor::ConfigField {
        key: "interval_minutes",
        label: "Intervallo (minuti)",
        help: "",
        kind: crate::connectors::descriptor::FieldKind::Number { min: 1, max: 60 },
        required: false,
    };
    static FIELDS: [crate::connectors::descriptor::ConfigField; 1] = [FIELD];
    static MINUTES: ConnectorDescriptor = ConnectorDescriptor {
        config_fields: &FIELDS,
        limits: DescriptorLimits { items_per_round: None, min_interval_secs: None, default_interval_secs: Some(120) },
        ..FAKE_SOURCE
    };
    let secs = |c| source_interval(&MINUTES, &c).as_secs();
    assert_eq!(secs(json!({"interval_minutes": 5})), 300);
    assert_eq!(secs(json!({"interval_minutes": 1})), 60);
    assert_eq!(secs(json!({})), 120, "il default del descrittore");
    assert_eq!(secs(json!({"interval_minutes": "x"})), 120, "non un numero: il default");
    assert_eq!(secs(json!({"interval_minutes": u64::MAX})), u64::MAX, "nessun overflow");
    // Il campo non dichiarato viene ignorato: vale il default.
    assert_eq!(secs(json!({"interval_secs": 900})), 120);
    // Il minimo del descrittore vince anche sul default.
    static FLOORED: ConnectorDescriptor = ConnectorDescriptor {
        limits: DescriptorLimits {
            items_per_round: None,
            min_interval_secs: Some(600),
            default_interval_secs: Some(120),
        },
        ..MINUTES
    };
    assert_eq!(source_interval(&FLOORED, &json!({})).as_secs(), 600);
    // Senza default e senza campi: il minimo del runtime.
    assert_eq!(source_interval(&FAKE_SOURCE, &json!({"interval_minutes": 9})), MIN_INTERVAL);
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
    assert_eq!(delivered_ids(&rig), vec!["y"]);
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), Some(&json!(5)));
}

/// Gli id esterni consegnati all'HUB, in ordine (anche quelli rifiutati).
fn delivered_ids(rig: &Rig) -> Vec<String> {
    rig.hub
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            HubCall::Deliver(d) => Some(d.origin.external_id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_round_failed_midway_saves_what_was_delivered_and_a_restart_does_not_repeat_it() {
    let conversations = route("rte-1-1", RouteThread::PerConversation, RouteWake::None);
    let rig = Rig::source(0, vec![conversations]);
    // La terza consegna fallisce: le prime due sono gia' nell'HUB.
    *rig.hub.fail_from.lock().unwrap() = Some(3);
    let source = new_source();
    let items = vec![item("rte-1-1", "a", Some("k1")), item("rte-1-1", "b", Some("k2")), item("rte-1-1", "c", None)];
    source.push(Ok(batch(items.clone(), json!(5), false)));
    rig.run_source(source, json!({}));
    assert_eq!(rig.status().state, HealthState::Backoff);
    let saved = rig.states.get(INSTANCE);
    assert!(saved.is_seen("rte-1-1", "a") && saved.is_seen("rte-1-1", "b"));
    assert!(!saved.is_seen("rte-1-1", "c"));
    let mapped = |key: &str| saved.conversations.get(&conversation_key("rte-1-1", key)).map(str::to_string);
    assert_eq!((mapped("k1"), mapped("k2")), (Some("thr-fake-1".to_string()), Some("thr-fake-2".to_string())));
    assert_eq!(saved.cursor(MAIN_CURSOR), None, "il cursore resta il vecchio");

    // Uscita dall'app durante il backoff e riavvio: il vecchio cursore ripresenta tutto.
    *rig.hub.fail_from.lock().unwrap() = None;
    source.push(Ok(batch(items, json!(5), false)));
    rig.run_source(source, json!({}));
    assert_eq!(delivered_ids(&rig), vec!["a", "b", "c", "c"], "a e b non si consegnano di nuovo");
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), Some(&json!(5)));
}

#[test]
fn a_round_that_disables_a_route_saves_what_was_delivered_before() {
    let gone = route("rte-1-2", RouteThread::Existing { thread_id: "thr-sparito".to_string() }, RouteWake::None);
    let rig = Rig::source(0, vec![per_item("rte-1-1"), gone]);
    let source = new_source();
    source.push(Ok(batch(vec![item("rte-1-1", "a", None), item("rte-1-2", "x", None)], json!(5), false)));
    // Il secondo giro (subito, dopo `Retry`) fallisce: resta solo lo stato del primo.
    source.push(transient());
    rig.run_source(source, json!({}));
    let saved = rig.states.get(INSTANCE);
    assert!(saved.is_seen("rte-1-1", "a"));
    assert_eq!(saved.cursor(MAIN_CURSOR), None);
    assert_eq!(delivered_ids(&rig), vec!["a"]);
    assert_eq!(rig.states.saves(), 2, "uno per giro, anche per quello rifatto e quello fallito");
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
fn a_hub_failure_in_a_batch_is_reported_to_the_user_and_the_cursor_moves_on() {
    let rig = contact_rig(0);
    let contact = new_contact();
    *rig.hub.fail_with.lock().unwrap() = Some("HUB spento".to_string());
    contact.push(Ok(ReceiveBatch { messages: vec![inbound("m1", "u1", "ciao")], cursor: json!(10) }));
    rig.run_contact(contact);
    assert_eq!(contact.sent_texts(), vec!["Messaggio non consegnato: HUB spento".to_string()]);
    assert_eq!(
        rig.states.get(INSTANCE).cursor(MAIN_CURSOR),
        Some(&json!(10)),
        "il messaggio e' trattato: non si ripresenta"
    );
    assert!(!rig.history().contains(&HealthState::Backoff));
}

#[test]
fn an_auth_error_while_replying_keeps_the_old_cursor_and_ends_the_loop() {
    let rig = contact_rig(10);
    let contact = new_contact();
    *contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    let help = crate::connectors::testing::command("c1", "u1", crate::connectors::contracts::ContactCommand::Help);
    contact.push(Ok(ReceiveBatch { messages: vec![help], cursor: json!(10) }));
    rig.run_contact(contact);
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), None);
    assert_eq!(rig.status().state, HealthState::NeedsAuth);
}

#[test]
fn a_batch_failed_midway_saves_the_handled_messages_and_a_restart_does_not_repeat_them() {
    let rig = contact_rig(1);
    let contact = new_contact();
    // Il primo messaggio va nell'HUB, la risposta al comando che segue fallisce e ferma il lotto.
    *contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    let help = crate::connectors::testing::command("c1", "u1", crate::connectors::contracts::ContactCommand::Help);
    let messages = vec![inbound("m1", "u1", "ciao"), help];
    contact.push(Ok(ReceiveBatch { messages: messages.clone(), cursor: json!(10) }));
    rig.run_contact(contact);
    assert_eq!(rig.status().state, HealthState::NeedsAuth);
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), None, "il cursore resta il vecchio");
    assert!(rig.states.saves() >= 1, "lo stato del lotto fallito e' su disco");

    // Riavvio con le credenziali sistemate: il vecchio cursore ripresenta il lotto.
    *contact.send_error.lock().unwrap() = None;
    contact.push(Ok(ReceiveBatch { messages, cursor: json!(10) }));
    rig.run_contact(contact);
    let written = rig.hub.calls().into_iter().filter(|c| matches!(c, HubCall::SendUser(_))).count();
    assert_eq!(written, 1, "m1 non si scrive di nuovo");
    assert_eq!(rig.states.get(INSTANCE).cursor(MAIN_CURSOR), Some(&json!(10)));
    assert_eq!(contact.sent_texts().len(), 1, "solo la risposta al comando");
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

fn notice(n: usize) -> ContactNotice {
    contact_notice(&format!("msg-{n}"), &format!("corpo {n}"))
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
        session: ContactSession::default(),
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
    assert_eq!(texts, vec!["Generale › Piano\ncorpo 0", "Generale › Piano\ncorpo 1"]);
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
