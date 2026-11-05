//! Il contesto che il runtime da' a un connettore: solo cio' che il descrittore
//! dichiara. Offre la configurazione gia' migrata e validata, i segreti dei
//! suoi slot, il client HTTP con l'allowlist degli host, orologio e casualita'
//! iniettati, lo stato privato dell'istanza e, solo con la capacita' dichiarata,
//! l'accesso all'HUB (`HubPort`). Una capacita' mancante risponde "permesso non
//! dichiarato": un connettore non puo' chiedere piu' di quanto dichiara.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "il contesto e' usato solo dai connettori registrati: il registro e' vuoto")
)]

use super::{BoxFuture, ConnectorError};
use crate::connectors::descriptor::{ConnectorDescriptor, HubCapability};
use crate::connectors::runtime::http::ContextHttp;
use crate::connectors::secrets::InstanceSecrets;
use crate::connectors::state::InstanceState;
use crate::hub::attachments::IncomingFile;
use crate::hub::contact::{ChannelEntry, ThreadEntry};
use crate::hub::ingress::{DeliveryOutcome, ExternalDelivery, ExternalWake};
use crate::hub::send::UserMessage;
use crate::hub::{Channel, Message};
use serde_json::Value;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Byte casuali richiedibili in una volta.
const RANDOM_MAX: usize = 1024;

/// Orologio e casualita', iniettati: la logica si prova senza tempo ne' rete.
pub trait Clock: Send + Sync {
    /// Millisecondi Unix.
    fn now_ms(&self) -> i64;
    /// Riempie `buf` di byte casuali.
    fn fill_random(&self, buf: &mut [u8]) -> Result<(), String>;
}

/// L'orologio e la casualita' del sistema.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    fn fill_random(&self, buf: &mut [u8]) -> Result<(), String> {
        getrandom::fill(buf).map_err(|_| "casualità del sistema non disponibile".to_string())
    }
}

/// La porta verso l'HUB, come la vede il runtime: i test la sostituiscono con un
/// finto. Le implementazioni vere chiamano le funzioni bloccanti di `hub` fuori
/// dal main thread. Gli errori sono testi in italiano per l'utente.
pub trait HubPort: Send + Sync {
    fn deliver_external(&self, delivery: ExternalDelivery) -> BoxFuture<'_, Result<DeliveryOutcome, String>>;
    fn send_user_message(&self, message: UserMessage) -> BoxFuture<'_, Result<Message, String>>;
    fn send_user_files(&self, message: UserMessage, files: Vec<IncomingFile>)
        -> BoxFuture<'_, Result<Message, String>>;
    /// Il thread esiste ancora nel canale? Errore se non si puo' sapere (HUB spento): non e' "no".
    fn thread_exists<'a>(&'a self, channel: &'a Channel, thread_id: &'a str) -> BoxFuture<'a, Result<bool, String>>;
    /// I canali dell'HUB col loro nome (Generale per primo). Solo per il runtime dei
    /// canali di contatto (`/ws`): `ConnectorContext` non la espone ai connettori.
    fn channels(&self) -> BoxFuture<'_, Result<Vec<ChannelEntry>, String>>;
    /// I thread aperti di tutti i canali (`/threads`). Solo per il runtime.
    fn open_threads(&self) -> BoxFuture<'_, Result<Vec<ThreadEntry>, String>>;
    /// Apre un thread libero nel canale (`/new`) e ne ritorna l'id. Solo per il runtime.
    fn create_thread<'a>(&'a self, channel: &'a Channel, title: &'a str) -> BoxFuture<'a, Result<String, String>>;
}

/// Lo stato dell'istanza condiviso fra connettore e supervisore (che lo salva).
#[derive(Clone, Default)]
pub struct StateHandle(Arc<Mutex<InstanceState>>);

impl StateHandle {
    pub fn new(state: InstanceState) -> Self {
        Self(Arc::new(Mutex::new(state)))
    }

    /// Accesso completo, per il runtime. Va tenuto il tempo di una modifica.
    pub fn lock(&self) -> MutexGuard<'_, InstanceState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Le parti di un contesto, assemblate dal runtime.
pub struct ContextParts {
    pub descriptor: &'static ConnectorDescriptor,
    pub instance: String,
    pub config: Value,
    pub secrets: InstanceSecrets,
    pub http: ContextHttp,
    pub state: StateHandle,
    pub hub: Arc<dyn HubPort>,
    pub clock: Arc<dyn Clock>,
}

pub struct ConnectorContext {
    parts: ContextParts,
    /// Il codice monouso emesso dal runtime per l'azione `pair`.
    pairing_code: Option<String>,
}

impl ConnectorContext {
    pub fn new(parts: ContextParts) -> Self {
        Self { parts, pairing_code: None }
    }

    /// Il contesto di un'azione `pair`: porta il codice di abbinamento appena emesso.
    #[must_use]
    pub fn with_pairing_code(mut self, code: String) -> Self {
        self.pairing_code = Some(code);
        self
    }

    /// Il codice di abbinamento emesso per questa azione `pair` (`None` per ogni altra
    /// azione). E' monouso e va mostrato all'utente: il connettore puo' inserirlo in un
    /// invito (es. un link) restituito nel `detail`, ma non lo logga.
    pub fn pairing_code(&self) -> Option<&str> {
        self.pairing_code.as_deref()
    }

    pub fn descriptor(&self) -> &'static ConnectorDescriptor {
        self.parts.descriptor
    }

    /// L'id dell'istanza (`con-...`).
    pub fn instance(&self) -> &str {
        &self.parts.instance
    }

    /// La configurazione, gia' migrata e validata. Non contiene segreti.
    pub fn config(&self) -> &Value {
        &self.parts.config
    }

    pub fn http(&self) -> &ContextHttp {
        &self.parts.http
    }

    pub fn now(&self) -> i64 {
        self.parts.clock.now_ms()
    }

    /// `len` byte casuali (al piu' `RANDOM_MAX`).
    pub fn random_bytes(&self, len: usize) -> Result<Vec<u8>, ConnectorError> {
        if len > RANDOM_MAX {
            return Err(ConnectorError::Fatal(format!("al più {RANDOM_MAX} byte casuali per volta")));
        }
        let mut buf = vec![0u8; len];
        self.parts.clock.fill_random(&mut buf).map_err(ConnectorError::Fatal)?;
        Ok(buf)
    }

    fn declared_slot(&self, slot: &str) -> Result<(), ConnectorError> {
        if self.parts.descriptor.declares_slot(slot) {
            Ok(())
        } else {
            Err(ConnectorError::Fatal("slot dei segreti non dichiarato".to_string()))
        }
    }

    /// Il segreto di uno slot dichiarato dal descrittore (campo `secret` o slot interno).
    pub fn secret(&self, slot: &str) -> Result<Option<String>, ConnectorError> {
        self.declared_slot(slot)?;
        self.parts.secrets.get(slot).map_err(ConnectorError::Transient)
    }

    /// Salva il segreto di uno slot dichiarato (es. il refresh token dopo `authorize`).
    pub fn set_secret(&self, slot: &str, value: &str) -> Result<(), ConnectorError> {
        self.declared_slot(slot)?;
        self.parts.secrets.set(slot, value).map_err(ConnectorError::Fatal)
    }

    /// Un valore privato del connettore nello stato dell'istanza.
    pub fn state_get(&self, key: &str) -> Option<Value> {
        self.parts.state.lock().custom_get(key).cloned()
    }

    /// Salva un valore privato (il runtime lo scrive su disco con lo stato).
    pub fn state_set(&self, key: &str, value: Value) -> Result<(), ConnectorError> {
        self.parts.state.lock().custom_set(key, value).map_err(ConnectorError::Fatal)
    }

    fn require(&self, capability: HubCapability) -> Result<(), ConnectorError> {
        if self.parts.descriptor.has_capability(capability) {
            Ok(())
        } else {
            Err(ConnectorError::Fatal(format!("permesso non dichiarato: {}", capability.as_str())))
        }
    }

    /// Autore e origine di un messaggio devono essere di questo connettore e di questa istanza.
    fn owns_origin(&self, connector: &str, instance: &str) -> Result<(), ConnectorError> {
        if connector == self.parts.descriptor.id && instance == self.parts.instance {
            Ok(())
        } else {
            Err(ConnectorError::Fatal("origine del messaggio non propria".to_string()))
        }
    }

    /// Consegna un elemento esterno nell'HUB. Richiede `deliver_messages`; senza
    /// `wake_secretaries` il risveglio e' ignorato. L'origine deve essere di questa istanza.
    pub async fn deliver(&self, mut delivery: ExternalDelivery) -> Result<DeliveryOutcome, ConnectorError> {
        self.require(HubCapability::DeliverMessages)?;
        self.owns_origin(&delivery.origin.connector, &delivery.origin.instance)?;
        if !self.parts.descriptor.has_capability(HubCapability::WakeSecretaries) {
            delivery.wake = ExternalWake::None;
        }
        self.parts.hub.deliver_external(delivery).await.map_err(ConnectorError::Transient)
    }

    /// Scrive a un thread come l'utente. Richiede `send_as_user`; `origin` e' obbligatoria
    /// e deve essere di questa istanza.
    pub async fn send_as_user(&self, message: UserMessage) -> Result<Message, ConnectorError> {
        self.check_user_message(&message)?;
        self.parts.hub.send_user_message(message).await.map_err(ConnectorError::Transient)
    }

    /// Come `send_as_user`, con gli allegati come byte.
    pub async fn send_as_user_files(
        &self,
        message: UserMessage,
        files: Vec<IncomingFile>,
    ) -> Result<Message, ConnectorError> {
        self.check_user_message(&message)?;
        self.parts.hub.send_user_files(message, files).await.map_err(ConnectorError::Transient)
    }

    fn check_user_message(&self, message: &UserMessage) -> Result<(), ConnectorError> {
        self.require(HubCapability::SendAsUser)?;
        match &message.origin {
            Some(origin) => self.owns_origin(&origin.connector, &origin.instance),
            None => Err(ConnectorError::Fatal("origine del messaggio mancante".to_string())),
        }
    }
}
