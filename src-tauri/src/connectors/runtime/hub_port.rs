//! La porta verso l'HUB vera: ogni chiamata gira nel pool bloccante
//! (`crate::blocking`) perche' le funzioni dell'HUB scrivono su disco. Non ha
//! logica propria: i connettori e i supervisori parlano col trait `HubPort`,
//! cosi' i test usano un finto senza `AppHandle`.

use crate::connectors::contracts::{BoxFuture, HubPort};
use crate::hub::attachments::IncomingFile;
use crate::hub::ingress::{deliver_external, DeliveryOutcome, ExternalDelivery};
use crate::hub::send::{send_user_files, send_user_message, UserMessage};
use crate::hub::{thread_exists, Channel, Message};
use tauri::AppHandle;

pub struct TauriHubPort(pub AppHandle);

impl HubPort for TauriHubPort {
    fn deliver_external(&self, delivery: ExternalDelivery) -> BoxFuture<'_, Result<DeliveryOutcome, String>> {
        let app = self.0.clone();
        Box::pin(crate::blocking(move || deliver_external(&app, delivery)))
    }

    fn send_user_message(&self, message: UserMessage) -> BoxFuture<'_, Result<Message, String>> {
        let app = self.0.clone();
        Box::pin(crate::blocking(move || send_user_message(&app, message)))
    }

    fn send_user_files(
        &self,
        message: UserMessage,
        files: Vec<IncomingFile>,
    ) -> BoxFuture<'_, Result<Message, String>> {
        let app = self.0.clone();
        Box::pin(crate::blocking(move || send_user_files(&app, message, files)))
    }

    fn thread_exists<'a>(&'a self, channel: &'a Channel, thread_id: &'a str) -> BoxFuture<'a, Result<bool, String>> {
        let app = self.0.clone();
        let (channel, thread_id) = (channel.clone(), thread_id.to_string());
        Box::pin(crate::blocking(move || thread_exists(&app, &channel, &thread_id)))
    }
}
