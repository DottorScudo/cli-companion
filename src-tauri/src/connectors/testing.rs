//! Strumenti dei test dei connettori: due connettori finti (una fonte e un canale
//! di contatto), una porta HUB finta che registra le chiamate, un orologio finto e
//! un contesto assemblato con segreti in memoria. Servono alla conformita', ai
//! test dei contratti e, nelle fasi successive, a quelli del supervisore: nessuna rete.

use super::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, Clock, Connector, ConnectorContext, ConnectorError, ConnectorImpl,
    ContextParts, PollBatch, SourceConnector, StateHandle,
};
use super::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, DescriptorLimits, FieldKind, HubCapability,
    Permissions,
};
use super::runtime::http::{shared_client, ContextHttp};
use super::secrets::InstanceSecrets;
use super::state::InstanceState;
use crate::hub::Author;
use crate::secrets::MemorySecrets;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const INSTANCE: &str = "con-1700000000000-1";

pub static FAKE_SOURCE: ConnectorDescriptor = ConnectorDescriptor {
    id: "fake-source",
    kind: ConnectorKind::Source,
    label: "Fonte finta",
    description: "Fonte di prova dei test.",
    version: 2,
    config_fields: &[
        ConfigField { key: "api_token", label: "Token", help: "", required: true, kind: FieldKind::Secret },
        ConfigField {
            key: "mailbox",
            label: "Casella",
            help: "",
            required: false,
            kind: FieldKind::Text { max_len: 50, placeholder: "" },
        },
        ConfigField {
            key: "interval_secs",
            label: "Intervallo",
            help: "",
            required: false,
            kind: FieldKind::Number { min: 60, max: 3600 },
        },
    ],
    filter_fields: &[ConfigField {
        key: "query",
        label: "Ricerca",
        help: "",
        required: true,
        kind: FieldKind::Text { max_len: 100, placeholder: "" },
    }],
    actions: &[Action { id: ActionId::Test, label: "Prova", help: "" }],
    permissions: Permissions {
        hosts: &["api.fake-source.example"],
        hub: &[HubCapability::DeliverMessages, HubCapability::WakeSecretaries],
    },
    limits: DescriptorLimits::NONE,
    internal_secret_slots: &["refresh_token"],
};

pub static FAKE_CONTACT: ConnectorDescriptor = ConnectorDescriptor {
    id: "fake-contact",
    kind: ConnectorKind::Contact,
    label: "Contatto finto",
    description: "Canale di contatto di prova dei test.",
    version: 1,
    config_fields: &[ConfigField {
        key: "bot_token",
        label: "Token del bot",
        help: "",
        required: true,
        kind: FieldKind::Secret,
    }],
    filter_fields: &[],
    actions: &[
        Action { id: ActionId::Test, label: "Prova", help: "" },
        Action { id: ActionId::Pair, label: "Abbina un account", help: "" },
    ],
    permissions: Permissions {
        hosts: &["api.fake-contact.example"],
        hub: &[HubCapability::ReceiveNotices, HubCapability::SendAsUser],
    },
    limits: DescriptorLimits::NONE,
    internal_secret_slots: &[],
};

/// Una fonte finta: `poll` restituisce i risultati preparati con `script`, poi
/// pagine vuote con un cursore che avanza.
pub struct FakeSource {
    script: Mutex<VecDeque<Result<PollBatch, ConnectorError>>>,
    polls: AtomicU64,
}

impl FakeSource {
    pub const fn new() -> Self {
        Self { script: Mutex::new(VecDeque::new()), polls: AtomicU64::new(0) }
    }

    pub fn push(&self, result: Result<PollBatch, ConnectorError>) {
        self.script.lock().unwrap().push_back(result);
    }

    pub fn polls(&self) -> u64 {
        self.polls.load(Ordering::SeqCst)
    }
}

impl Connector for FakeSource {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        &FAKE_SOURCE
    }

    /// v1 -> v2: aggiunge `interval_secs` (60 se manca).
    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String> {
        let Value::Object(mut map) = config else { return Err("configurazione non valida".to_string()) };
        match from {
            1 => {
                map.entry("interval_secs").or_insert(json!(60));
                Ok(Value::Object(map))
            }
            _ => Err(format!("nessuna migrazione dalla versione {from}")),
        }
    }

    fn validate_config(&self, config: &Value) -> Result<(), String> {
        match config.get("mailbox").and_then(Value::as_str) {
            Some("vietata") => Err("casella non ammessa".to_string()),
            _ => Ok(()),
        }
    }

    fn action<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        action: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(async move {
            match action {
                ActionId::Test => Ok(ActionOutcome { message: "Connessione riuscita.".to_string(), detail: None }),
                _ => Err(ConnectorError::Fatal("azione non dichiarata".to_string())),
            }
        })
    }
}

impl SourceConnector for FakeSource {
    fn validate_filter(&self, filter: &Value) -> Result<(), String> {
        match filter.get("query").and_then(Value::as_str) {
            Some("*") => Err("ricerca troppo ampia".to_string()),
            _ => Ok(()),
        }
    }

    fn poll<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        _routes: &'a [ActiveRoute],
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<PollBatch, ConnectorError>> {
        Box::pin(async move {
            let n = self.polls.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(result) = self.script.lock().unwrap().pop_front() {
                return result;
            }
            let next = cursor.and_then(|c| c.as_u64()).unwrap_or(0) + 1;
            Ok(PollBatch { items: Vec::new(), cursor: json!(next.max(n)), more: false })
        })
    }
}

pub static FAKE_SOURCE_CONNECTOR: FakeSource = FakeSource::new();
pub static FAKE_CONTACT_CONNECTOR: FakeContact = FakeContact::new();

/// I due connettori finti, come li registrerebbe `registry`.
pub fn fakes() -> Vec<ConnectorImpl> {
    vec![ConnectorImpl::Source(&FAKE_SOURCE_CONNECTOR), ConnectorImpl::Contact(&FAKE_CONTACT_CONNECTOR)]
}

/// Orologio finto: il tempo lo muove il test; i byte "casuali" sono una sequenza.
pub struct FakeClock {
    now: AtomicI64,
    counter: AtomicU64,
}

impl FakeClock {
    pub fn new(now: i64) -> Self {
        Self { now: AtomicI64::new(now), counter: AtomicU64::new(0) }
    }

    pub fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }

    fn fill_random(&self, buf: &mut [u8]) -> Result<(), String> {
        for byte in buf {
            *byte = (self.counter.fetch_add(1, Ordering::SeqCst) % 251) as u8;
        }
        Ok(())
    }
}

/// Un contesto per `descriptor` con config vuota, segreti in memoria, porta HUB e
/// orologio finti. Restituisce anche i pezzi per ispezionarli.
pub struct TestContext {
    pub ctx: ConnectorContext,
    pub hub: Arc<FakeHub>,
    pub clock: Arc<FakeClock>,
    pub secrets: Arc<MemorySecrets>,
    pub state: StateHandle,
}

pub fn test_context(descriptor: &'static ConnectorDescriptor, config: Value) -> TestContext {
    let hub = Arc::new(FakeHub::default());
    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let secrets = Arc::new(MemorySecrets::default());
    let state = StateHandle::new(InstanceState::default());
    let ctx = ConnectorContext::new(ContextParts {
        descriptor,
        instance: INSTANCE.to_string(),
        config,
        secrets: InstanceSecrets::new(secrets.clone(), INSTANCE).unwrap(),
        http: ContextHttp::new(shared_client().unwrap(), descriptor.permissions.hosts),
        state: state.clone(),
        hub: hub.clone(),
        clock: clock.clone(),
    });
    TestContext { ctx, hub, clock, secrets, state }
}

/// Un autore esterno di prova, coerente con `INSTANCE` e `FAKE_SOURCE`.
pub fn external_author() -> Author {
    Author::External {
        connector: FAKE_SOURCE.id.to_string(),
        instance: INSTANCE.to_string(),
        name: "Mario".to_string(),
    }
}

mod contact;
mod hub;
mod kit;
pub use contact::*;
pub use hub::*;
pub use kit::*;
