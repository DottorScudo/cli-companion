//! La porta HUB finta: registra le chiamate, tiene i thread esistenti e gli
//! elenchi di canali e thread aperti, e risponde con id progressivi.

use crate::connectors::contracts::{BoxFuture, HubPort};
use crate::hub::attachments::IncomingFile;
use crate::hub::contact::{ChannelEntry, ThreadEntry};
use crate::hub::ingress::{DeliveryOutcome, DeliveryTarget, ExternalDelivery};
use crate::hub::send::UserMessage;
use crate::hub::{Channel, Message};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Mutex;

/// Una chiamata fatta alla porta HUB finta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubCall {
    Deliver(ExternalDelivery),
    SendUser(UserMessage),
    SendUserFiles(UserMessage, Vec<IncomingFile>),
    CreateThread { channel: Channel, title: String },
}

/// La porta HUB finta: registra le chiamate e risponde con id progressivi.
#[derive(Default)]
pub struct FakeHub {
    pub calls: Mutex<Vec<HubCall>>,
    pub fail_with: Mutex<Option<String>>,
    pub threads: Mutex<HashSet<(String, String)>>,
    /// Dalla N-esima chiamata (da 1) ogni chiamata fallisce: prova il fallimento a meta' di un giro.
    pub fail_from: Mutex<Option<usize>>,
    /// I canali che `channels` elenca (vuoto = nessuno: il test li imposta).
    pub channel_list: Mutex<Vec<ChannelEntry>>,
    /// I thread aperti che `open_threads` elenca.
    pub open: Mutex<Vec<ThreadEntry>>,
    /// Le chiamate (da 1) che falliscono, le altre riescono: prova un nuovo tentativo.
    pub fail_at: Mutex<Vec<usize>>,
    /// Se impostato, `channels` e `open_threads` falliscono con questo errore.
    pub list_error: Mutex<Option<String>>,
}

impl FakeHub {
    pub fn calls(&self) -> Vec<HubCall> {
        self.calls.lock().unwrap().clone()
    }

    fn failure(&self, call: usize) -> Option<String> {
        let from = *self.fail_from.lock().unwrap();
        let late = from.is_some_and(|from| call >= from).then(|| "HUB non disponibile".to_string());
        let once = self.fail_at.lock().unwrap().contains(&call).then(|| "HUB ha rifiutato".to_string());
        self.fail_with.lock().unwrap().clone().or(late).or(once)
    }

    fn record(&self, call: HubCall) -> usize {
        let mut calls = self.calls.lock().unwrap();
        calls.push(call);
        calls.len()
    }

    pub fn add_thread(&self, channel: &Channel, thread_id: &str) {
        self.threads.lock().unwrap().insert((channel.as_str().to_string(), thread_id.to_string()));
    }

    /// Un thread aperto: compare in `open_threads` ed esiste per `thread_exists`.
    pub fn add_open_thread(&self, entry: ThreadEntry) {
        self.add_thread(&entry.channel, &entry.thread_id);
        self.open.lock().unwrap().push(entry);
    }

    pub fn set_channels(&self, channels: Vec<ChannelEntry>) {
        *self.channel_list.lock().unwrap() = channels;
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

    fn channels(&self) -> BoxFuture<'_, Result<Vec<ChannelEntry>, String>> {
        Box::pin(async move {
            match self.list_error.lock().unwrap().clone() {
                Some(e) => Err(e),
                None => Ok(self.channel_list.lock().unwrap().clone()),
            }
        })
    }

    fn open_threads(&self) -> BoxFuture<'_, Result<Vec<ThreadEntry>, String>> {
        Box::pin(async move {
            match self.list_error.lock().unwrap().clone() {
                Some(e) => Err(e),
                None => Ok(self.open.lock().unwrap().clone()),
            }
        })
    }

    fn create_thread<'a>(&'a self, channel: &'a Channel, title: &'a str) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let n = self.record(HubCall::CreateThread { channel: channel.clone(), title: title.to_string() });
            if let Some(e) = self.failure(n) {
                return Err(e);
            }
            if title.trim().is_empty() {
                return Err("il titolo deve avere da 1 a 120 caratteri".to_string());
            }
            let thread_id = format!("thr-new-{n}");
            self.add_thread(channel, &thread_id);
            Ok(thread_id)
        })
    }
}
