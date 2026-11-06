//! Il canale di contatto finto: registra i messaggi mandati, le risposte di
//! abbinamento e le sotto-chat aperte, e simula una chat con le sotto-chat (aperte
//! con id progressivi, che possono fallire o sparire), cosi' il runtime si prova da
//! capo a fondo senza un servizio vero.

use super::FAKE_CONTACT;
use crate::connectors::contracts::{
    ActionOutcome, BoxFuture, Connector, ConnectorContext, ConnectorError, ContactConnector, OutboundMessage,
    PairingOutcome, ReceiveBatch, RemoteFile,
};
use crate::connectors::descriptor::{ActionId, ConnectorDescriptor};
use crate::hub::attachments::IncomingFile;
use crate::hub::NoticeCategory;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

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
    /// Le chat che hanno le sotto-chat.
    subchat_chats: Mutex<Vec<String>>,
    /// Le sotto-chat aperte, `(chat, titolo)`, in ordine.
    pub opened: Mutex<Vec<(String, String)>>,
    /// Se impostato, `open_subchat` fallisce con questo errore.
    pub open_error: Mutex<Option<ConnectorError>>,
    /// Se impostato, il prossimo `open_subchat` ritorna questo id (anche non valido).
    next_subchat_id: Mutex<Option<String>>,
    /// Le sotto-chat eliminate nel servizio: `send` verso di loro risponde `SubchatGone`.
    gone: Mutex<Vec<String>>,
    next_subchat: AtomicU64,
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
            subchat_chats: Mutex::new(Vec::new()),
            opened: Mutex::new(Vec::new()),
            open_error: Mutex::new(None),
            next_subchat_id: Mutex::new(None),
            gone: Mutex::new(Vec::new()),
            next_subchat: AtomicU64::new(0),
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

    /// Da ora la `chat` ha le sotto-chat.
    pub fn with_subchats_in(&self, chat: &str) {
        self.subchat_chats.lock().unwrap().push(chat.to_string());
    }

    /// Il prossimo `open_subchat` ritorna `id`.
    pub fn script_subchat_id(&self, id: &str) {
        *self.next_subchat_id.lock().unwrap() = Some(id.to_string());
    }

    /// L'utente elimina la sotto-chat nel servizio.
    pub fn delete_remotely(&self, subchat: &str) {
        self.gone.lock().unwrap().push(subchat.to_string());
    }

    /// I titoli delle sotto-chat aperte, in ordine.
    pub fn opened_titles(&self) -> Vec<String> {
        self.opened.lock().unwrap().iter().map(|(_, title)| title.clone()).collect()
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
            if out.subchat.as_ref().is_some_and(|s| self.gone.lock().unwrap().contains(s)) {
                return Err(ConnectorError::SubchatGone);
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

    fn subchats(&self, _config: &Value, chat: &str) -> bool {
        self.subchat_chats.lock().unwrap().iter().any(|c| c == chat)
    }

    fn open_subchat<'a>(
        &'a self,
        _ctx: &'a ConnectorContext,
        chat: &'a str,
        title: &'a str,
    ) -> BoxFuture<'a, Result<String, ConnectorError>> {
        Box::pin(async move {
            if let Some(error) = self.open_error.lock().unwrap().clone() {
                return Err(error);
            }
            self.opened.lock().unwrap().push((chat.to_string(), title.to_string()));
            let scripted = self.next_subchat_id.lock().unwrap().take();
            Ok(scripted.unwrap_or_else(|| format!("sub-{}", self.next_subchat.fetch_add(1, Ordering::SeqCst) + 1)))
        })
    }
}
