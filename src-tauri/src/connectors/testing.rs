//! Strumenti dei test dei connettori: due connettori finti (una fonte e un canale
//! di contatto), una porta HUB finta che registra le chiamate, un orologio finto e
//! un contesto assemblato con segreti in memoria. Servono alla conformita', ai
//! test dei contratti e, nelle fasi successive, a quelli del supervisore: nessuna rete.

use super::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, Clock, Connector, ConnectorContext, ConnectorError, ConnectorImpl,
    ContactConnector, ContextParts, OutboundMessage, PairingOutcome, PollBatch, ReceiveBatch, RemoteFile,
    SourceConnector, StateHandle,
};
use super::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, DescriptorLimits, FieldKind, HubCapability,
    Permissions,
};
use super::runtime::http::{shared_client, ContextHttp};
use super::secrets::InstanceSecrets;
use super::state::InstanceState;
use crate::hub::attachments::IncomingFile;
use crate::hub::{Author, NoticeCategory};
use crate::secrets::MemorySecrets;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
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

/// L'esito di `fetch_file` per un id: i byte o un errore.
type ScriptedFile = (String, Result<Vec<u8>, ConnectorError>);

/// Un canale di contatto finto: registra i messaggi mandati e le risposte di abbinamento.
pub struct FakeContact {
    script: Mutex<VecDeque<Result<ReceiveBatch, ConnectorError>>>,
    pub sent: Mutex<Vec<OutboundMessage>>,
    pub pairing: Mutex<Vec<(PairingOutcome, String)>>,
    /// Se impostato, `send` fallisce con questo errore.
    pub send_error: Mutex<Option<ConnectorError>>,
    /// I tipi di notifica che `wants_notice` rifiuta.
    refused: Mutex<Vec<NoticeCategory>>,
    /// In quante parti `send` spezza ogni messaggio (almeno una): un id per parte.
    parts: AtomicUsize,
    next_id: AtomicU64,
    /// I file che `fetch_file` sa scaricare, per id.
    files: Mutex<Vec<ScriptedFile>>,
    /// Gli id dei file chiesti a `fetch_file`, in ordine.
    pub fetched: Mutex<Vec<String>>,
}

impl FakeContact {
    pub const fn new() -> Self {
        Self {
            script: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            pairing: Mutex::new(Vec::new()),
            send_error: Mutex::new(None),
            refused: Mutex::new(Vec::new()),
            parts: AtomicUsize::new(1),
            next_id: AtomicU64::new(0),
            files: Mutex::new(Vec::new()),
            fetched: Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, result: Result<ReceiveBatch, ConnectorError>) {
        self.script.lock().unwrap().push_back(result);
    }

    /// Da ora `wants_notice` rifiuta questo tipo.
    pub fn refuse(&self, category: NoticeCategory) {
        self.refused.lock().unwrap().push(category);
    }

    /// Ogni `send` si spezza in `parts` parti.
    pub fn split_sends_in(&self, parts: usize) {
        self.parts.store(parts, Ordering::SeqCst);
    }

    /// Prepara l'esito di `fetch_file` per l'id `file_id`.
    pub fn script_file(&self, file_id: &str, result: Result<Vec<u8>, ConnectorError>) {
        self.files.lock().unwrap().push((file_id.to_string(), result));
    }

    /// I testi mandati, in ordine.
    pub fn sent_texts(&self) -> Vec<String> {
        self.sent.lock().unwrap().iter().map(|m| m.text.clone()).collect()
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
    fn wants_notice(&self, _config: &Value, category: NoticeCategory) -> bool {
        !self.refused.lock().unwrap().contains(&category)
    }

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
    ) -> BoxFuture<'a, Result<Vec<String>, ConnectorError>> {
        Box::pin(async move {
            if let Some(error) = self.send_error.lock().unwrap().clone() {
                return Err(error);
            }
            self.sent.lock().unwrap().push(out.clone());
            let parts = self.parts.load(Ordering::SeqCst).max(1);
            Ok((0..parts).map(|_| format!("ext-{}", self.next_id.fetch_add(1, Ordering::SeqCst) + 1)).collect())
        })
    }

    fn fetch_file<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        file: &'a RemoteFile,
    ) -> BoxFuture<'a, Result<IncomingFile, ConnectorError>> {
        Box::pin(async move {
            self.fetched.lock().unwrap().push(file.id.clone());
            let scripted = self.files.lock().unwrap().iter().find(|(id, _)| *id == file.id).map(|(_, r)| r.clone());
            match scripted {
                Some(Ok(bytes)) => Ok(IncomingFile { name: file.name.clone(), bytes }),
                Some(Err(error)) => Err(error),
                None => Err(ConnectorError::Transient("file non disponibile".to_string())),
            }
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

mod hub;
mod kit;
pub use hub::*;
pub use kit::*;
