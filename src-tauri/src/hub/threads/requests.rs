//! Le richieste di azione dei segretari dentro i messaggi di un thread (vedi
//! `engine::confirm`): la scheda si aggiorna sul posto, cosi' mostra sempre
//! lo stato corrente della richiesta.

use super::ChannelThreads;
use crate::hub::model::{HubActionRequest, HubEventKind};

const REQUEST_NOT_FOUND: &str = "richiesta non trovata";

impl ChannelThreads {
    /// Applica `change` alla richiesta della scheda `message_id` del thread e
    /// ne ritorna l'esito. Aggiorna `updated_at` del thread.
    pub fn update_request<R>(
        &mut self,
        thread_id: &str,
        message_id: &str,
        now: i64,
        change: impl FnOnce(&mut HubActionRequest) -> R,
    ) -> Result<R, String> {
        let thread = self.thread_mut(thread_id)?;
        let request = thread
            .messages
            .iter_mut()
            .filter(|m| m.id == message_id)
            .filter_map(|m| m.event.as_mut())
            .filter(|e| e.kind == HubEventKind::ActionRequest)
            .find_map(|e| e.request.as_mut())
            .ok_or_else(|| REQUEST_NOT_FOUND.to_string())?;
        let out = change(request);
        thread.updated_at = now;
        Ok(out)
    }
}
