//! Attrezzi dei test del runtime: costruttori di regole, elementi e istanze, un
//! "dormitore" istantaneo, lo stato in memoria e un registratore degli eventi di
//! salute. Ogni test costruisce i suoi connettori finti (`new_source`,
//! `new_contact`): i `static` condivisi di `testing.rs` hanno un copione unico e
//! test in parallelo se lo contenderebbero.

use super::{FakeContact, FakeSource, FAKE_SOURCE, INSTANCE};
use crate::connectors::contracts::{BoxFuture, InboundMessage, PollBatch, SourceItem};
use crate::connectors::descriptor::{ConnectorDescriptor, HubCapability, Permissions};
use crate::connectors::model::{
    ConnectorInstance, ConnectorLink, ConnectorNotices, ConnectorRoute, LinkThread, RouteThread, RouteWake,
};
use crate::connectors::runtime::health::{ConnectorStatus, StatusEmitter};
use crate::connectors::runtime::store::StateStore;
use crate::connectors::runtime::time::Sleeper;
use crate::connectors::state::InstanceState;
use crate::hub::Channel;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub fn new_source() -> &'static FakeSource {
    Box::leak(Box::new(FakeSource::new()))
}

pub fn new_contact() -> &'static FakeContact {
    Box::leak(Box::new(FakeContact::new()))
}

/// Come `FAKE_SOURCE`, ma senza `wake_secretaries`.
pub static NO_WAKE_SOURCE: ConnectorDescriptor = ConnectorDescriptor {
    permissions: Permissions { hosts: &["api.fake-source.example"], hub: &[HubCapability::DeliverMessages] },
    ..FAKE_SOURCE
};

pub fn route(id: &str, thread: RouteThread, wake: RouteWake) -> ConnectorRoute {
    ConnectorRoute {
        id: id.to_string(),
        instance: INSTANCE.to_string(),
        label: "regola".to_string(),
        filter: json!({"query": "x"}),
        channel: Channel::General,
        thread,
        wake,
        enabled: true,
        error: None,
        created_at: 1,
        updated_at: 1,
    }
}

pub fn item(route: &str, external_id: &str, conversation: Option<&str>) -> SourceItem {
    SourceItem {
        route: route.to_string(),
        external_id: external_id.to_string(),
        conversation_key: conversation.map(str::to_string),
        title: format!("Titolo {external_id}"),
        sender: "Mario".to_string(),
        body: format!("corpo {external_id}"),
        files: Vec::new(),
    }
}

pub fn batch(items: Vec<SourceItem>, cursor: Value, more: bool) -> PollBatch {
    PollBatch { items, cursor, more }
}

pub fn instance(id: &str, connector: &str, config: Value, version: u32) -> ConnectorInstance {
    ConnectorInstance {
        id: id.to_string(),
        connector: connector.to_string(),
        label: "istanza".to_string(),
        enabled: true,
        config,
        config_version: version,
        notices: ConnectorNotices::default(),
        created_at: 1,
        updated_at: 1,
    }
}

pub fn inbound(external_id: &str, account: &str, text: &str) -> InboundMessage {
    InboundMessage {
        external_id: external_id.to_string(),
        account: account.to_string(),
        chat: format!("chat-{account}"),
        text: text.to_string(),
        reply_to_external: None,
        files: Vec::new(),
        pairing_code: None,
    }
}

pub fn link(id: &str, account: &str, current: Option<&str>) -> ConnectorLink {
    ConnectorLink {
        id: id.to_string(),
        instance: INSTANCE.to_string(),
        external_account: account.to_string(),
        external_chat: format!("chat-{account}"),
        display_name: account.to_string(),
        current: current.map(|t| LinkThread { channel: Channel::General, thread_id: t.to_string() }),
        muted_until: None,
        created_at: 1,
    }
}

/// Un "dormitore" che non aspetta: registra le attese e dopo `budget` attese chiede lo stop.
pub struct InstantSleeper {
    pub sleeps: Mutex<Vec<Duration>>,
    budget: AtomicUsize,
}

impl InstantSleeper {
    pub fn with_budget(budget: usize) -> Self {
        Self { sleeps: Mutex::new(Vec::new()), budget: AtomicUsize::new(budget) }
    }

    pub fn secs(&self) -> Vec<u64> {
        self.sleeps.lock().unwrap().iter().map(Duration::as_secs).collect()
    }
}

impl Sleeper for InstantSleeper {
    fn sleep(&self, duration: Duration) -> BoxFuture<'_, bool> {
        Box::pin(async move {
            self.sleeps.lock().unwrap().push(duration);
            let left = self.budget.load(Ordering::SeqCst);
            self.budget.store(left.saturating_sub(1), Ordering::SeqCst);
            left > 0
        })
    }
}

/// Un "dormitore" che non si sveglia mai: il supervisore resta vivo finche' non lo si ferma.
pub struct ParkedSleeper;

impl Sleeper for ParkedSleeper {
    fn sleep(&self, _: Duration) -> BoxFuture<'_, bool> {
        Box::pin(std::future::pending())
    }
}

/// Lo stato delle istanze in memoria, con il conteggio dei salvataggi.
#[derive(Default)]
pub struct MemoryStates {
    pub map: Mutex<HashMap<String, InstanceState>>,
    pub saves: AtomicUsize,
}

impl MemoryStates {
    pub fn get(&self, instance: &str) -> InstanceState {
        self.map.lock().unwrap().get(instance).cloned().unwrap_or_default()
    }

    pub fn saves(&self) -> usize {
        self.saves.load(Ordering::SeqCst)
    }
}

impl StateStore for MemoryStates {
    fn load<'a>(&'a self, instance: &'a str) -> BoxFuture<'a, InstanceState> {
        Box::pin(async move { self.get(instance) })
    }

    fn save<'a>(&'a self, instance: &'a str, state: &'a InstanceState) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.map.lock().unwrap().insert(instance.to_string(), state.clone());
            self.saves.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

/// Registra gli elenchi emessi con `connector-status`.
#[derive(Default)]
pub struct RecordingEmitter(pub Mutex<Vec<Vec<ConnectorStatus>>>);

impl StatusEmitter for RecordingEmitter {
    fn emit(&self, statuses: &[ConnectorStatus]) {
        self.0.lock().unwrap().push(statuses.to_vec());
    }
}
