//! Strumenti dei test dei connettori: due connettori finti (una fonte e un canale
//! di contatto), una porta HUB finta che registra le chiamate, un orologio finto e
//! un contesto assemblato con segreti in memoria. Servono alla conformita', ai
//! test dei contratti e, nelle fasi successive, a quelli del supervisore: nessuna rete.

use super::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, Clock, Connector, ConnectorContext, ConnectorError, ConnectorImpl,
    ContactConnector, ContextParts, HubPort, OutboundMessage, PairingOutcome, PollBatch, ReceiveBatch, SourceConnector,
    StateHandle,
};
use super::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, DescriptorLimits, FieldKind, HubCapability,
    Permissions,
};
use super::runtime::http::{shared_client, ContextHttp};
use super::secrets::InstanceSecrets;
use super::state::InstanceState;
use crate::hub::attachments::IncomingFile;
use crate::hub::ingress::{DeliveryOutcome, DeliveryTarget, ExternalDelivery};
use crate::hub::send::UserMessage;
use crate::hub::{Author, Channel, Message};
use crate::secrets::MemorySecrets;
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
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

/// Un canale di contatto finto: registra i messaggi mandati e le risposte di abbinamento.
pub struct FakeContact {
    script: Mutex<VecDeque<Result<ReceiveBatch, ConnectorError>>>,
    pub sent: Mutex<Vec<OutboundMessage>>,
    pub pairing: Mutex<Vec<(PairingOutcome, String)>>,
    /// Se impostato, `send` fallisce con questo errore.
    pub send_error: Mutex<Option<ConnectorError>>,
}

impl FakeContact {
    pub const fn new() -> Self {
        Self {
            script: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            pairing: Mutex::new(Vec::new()),
            send_error: Mutex::new(None),
        }
    }

    pub fn push(&self, result: Result<ReceiveBatch, ConnectorError>) {
        self.script.lock().unwrap().push_back(result);
    }
}

impl Connector for FakeContact {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        &FAKE_CONTACT
    }

    fn migrate_config(&self, from: u32, _config: Value) -> Result<Value, String> {
        Err(format!("nessuna migrazione dalla versione {from}"))
    }

    fn validate_config(&self, _config: &Value) -> Result<(), String> {
        Ok(())
    }

    fn action<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        action: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(async move {
            let detail = match ctx.pairing_code() {
                Some(code) => format!("invito:{code}"),
                None => "dettaglio".to_string(),
            };
            Ok(ActionOutcome { message: format!("azione {}", action.as_str()), detail: Some(detail) })
        })
    }
}

impl ContactConnector for FakeContact {
    fn receive<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<ReceiveBatch, ConnectorError>> {
        Box::pin(async move {
            if let Some(result) = self.script.lock().unwrap().pop_front() {
                return result;
            }
            Ok(ReceiveBatch { messages: Vec::new(), cursor: cursor.unwrap_or(Value::Null) })
        })
    }

    fn send<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        out: &'a OutboundMessage,
    ) -> BoxFuture<'a, Result<String, ConnectorError>> {
        Box::pin(async move {
            if let Some(error) = self.send_error.lock().unwrap().clone() {
                return Err(error);
            }
            let mut sent = self.sent.lock().unwrap();
            sent.push(out.clone());
            Ok(format!("ext-{}", sent.len()))
        })
    }

    fn pairing_reply<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        outcome: &'a PairingOutcome,
        chat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(async move {
            self.pairing.lock().unwrap().push((outcome.clone(), chat.to_string()));
            Ok(())
        })
    }
}

pub static FAKE_SOURCE_CONNECTOR: FakeSource = FakeSource::new();
pub static FAKE_CONTACT_CONNECTOR: FakeContact = FakeContact::new();

/// I due connettori finti, come li registrerebbe `registry`.
pub fn fakes() -> Vec<ConnectorImpl> {
    vec![ConnectorImpl::Source(&FAKE_SOURCE_CONNECTOR), ConnectorImpl::Contact(&FAKE_CONTACT_CONNECTOR)]
}

/// Una chiamata fatta alla porta HUB finta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubCall {
    Deliver(ExternalDelivery),
    SendUser(UserMessage),
    SendUserFiles(UserMessage, Vec<IncomingFile>),
}

/// La porta HUB finta: registra le chiamate e risponde con id progressivi.
#[derive(Default)]
pub struct FakeHub {
    pub calls: Mutex<Vec<HubCall>>,
    pub fail_with: Mutex<Option<String>>,
    pub threads: Mutex<HashSet<(String, String)>>,
    /// Dalla N-esima chiamata (da 1) ogni chiamata fallisce: prova il fallimento a meta' di un giro.
    pub fail_from: Mutex<Option<usize>>,
}

impl FakeHub {
    pub fn calls(&self) -> Vec<HubCall> {
        self.calls.lock().unwrap().clone()
    }

    fn failure(&self, call: usize) -> Option<String> {
        let from = *self.fail_from.lock().unwrap();
        let late = from.is_some_and(|from| call >= from).then(|| "HUB non disponibile".to_string());
        self.fail_with.lock().unwrap().clone().or(late)
    }

    fn record(&self, call: HubCall) -> usize {
        let mut calls = self.calls.lock().unwrap();
        calls.push(call);
        calls.len()
    }

    pub fn add_thread(&self, channel: &Channel, thread_id: &str) {
        self.threads.lock().unwrap().insert((channel.as_str().to_string(), thread_id.to_string()));
    }
}

fn fake_message(n: usize, body: &str) -> Message {
    serde_json::from_value(json!({
        "id": format!("msg-{n}"),
        "author": {"type": "user"},
        "kind": "text",
        "body": body,
        "created_at": 1
    }))
    .expect("messaggio finto valido")
}

impl HubPort for FakeHub {
    fn deliver_external(&self, delivery: ExternalDelivery) -> BoxFuture<'_, Result<DeliveryOutcome, String>> {
        Box::pin(async move {
            let channel = delivery.channel.clone();
            let existing = match &delivery.target {
                DeliveryTarget::Existing(id) => Some(id.clone()),
                DeliveryTarget::New { .. } => None,
            };
            let n = self.record(HubCall::Deliver(delivery));
            if let Some(e) = self.failure(n) {
                return Err(e);
            }
            // Un thread nuovo esiste da subito, come nell'HUB vero.
            let thread_id = existing.unwrap_or_else(|| format!("thr-fake-{n}"));
            self.add_thread(&channel, &thread_id);
            Ok(DeliveryOutcome { thread_id, message_id: format!("msg-{n}"), duplicate: false })
        })
    }

    fn send_user_message(&self, message: UserMessage) -> BoxFuture<'_, Result<Message, String>> {
        Box::pin(async move {
            let body = message.body.clone();
            let n = self.record(HubCall::SendUser(message));
            self.failure(n).map_or_else(|| Ok(fake_message(n, &body)), Err)
        })
    }

    fn send_user_files(
        &self,
        message: UserMessage,
        files: Vec<IncomingFile>,
    ) -> BoxFuture<'_, Result<Message, String>> {
        Box::pin(async move {
            let body = message.body.clone();
            let n = self.record(HubCall::SendUserFiles(message, files));
            self.failure(n).map_or_else(|| Ok(fake_message(n, &body)), Err)
        })
    }

    fn thread_exists<'a>(&'a self, channel: &'a Channel, thread_id: &'a str) -> BoxFuture<'a, Result<bool, String>> {
        Box::pin(async move {
            Ok(self.threads.lock().unwrap().contains(&(channel.as_str().to_string(), thread_id.to_string())))
        })
    }
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

mod kit;
pub use kit::*;
