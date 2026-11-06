//! L'eco dei messaggi che l'utente scrive nell'app (docs/CONNECTORS.MD §10, «Eco dei
//! messaggi scritti nell'app»): nella sotto-chat del thread, se esiste gia', cosi' la
//! sotto-chat e' la chat intera del thread. L'HUB manda qui solo i messaggi scritti
//! nell'app, mai quelli arrivati da un canale di contatto (nessuna eco infinita).
//! L'eco non apre sotto-chat (un thread mai notificato non riempie il gruppo) e non
//! va nelle chat senza sotto-chat. L'id esterno di ogni parte entra nella mappa: una
//! risposta all'eco torna nel thread citando il messaggio dell'utente.

use super::refs::MessageRef;
use super::subchats::{SubchatKey, ThreadKey};
use super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, OutboundMessage};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::time::with_timeout;
use crate::hub::AppMessage;

/// Il prefisso del testo dell'eco: dice che il messaggio e' dell'utente, scritto nell'app.
const ECHO_PREFIX: &str = "Tu (app): ";

/// La riga sugli allegati, di cui si cita solo il numero (i file non si mandano).
fn attachments_note(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("📎 1 allegato".to_string()),
        n => Some(format!("📎 {n} allegati")),
    }
}

/// Il testo dell'eco: `"Tu (app): <testo>"` e, se ci sono, sotto la riga degli allegati
/// (da sola, senza testo). `None` se non c'e' nulla da mostrare. Puro.
pub(super) fn echo_text(message: &AppMessage) -> Option<String> {
    let body = message.text.trim();
    let note = attachments_note(message.attachment_count);
    match (body.is_empty(), note) {
        (true, None) => None,
        (true, Some(note)) => Some(format!("{ECHO_PREFIX}{note}")),
        (false, None) => Some(format!("{ECHO_PREFIX}{body}")),
        (false, Some(note)) => Some(format!("{ECHO_PREFIX}{body}\n{note}")),
    }
}

impl ContactCtx<'_> {
    /// Rispecchia il messaggio nella sotto-chat del suo thread, in ogni chat dei collegamenti
    /// non silenziati che ha le sotto-chat e in cui il thread ha gia' la sua. Non fa nulla se
    /// il connettore non vuole l'eco (interruttore spento). Un errore del servizio che ferma
    /// il supervisore (credenziali, configurazione) risale; gli altri si registrano.
    pub async fn mirror_app_message(&self, message: &AppMessage) -> Result<(), ConnectorError> {
        if !self.connector.mirrors_app_messages(self.ctx.config()) {
            return Ok(());
        }
        let Some(text) = echo_text(message) else { return Ok(()) };
        let thread = ThreadKey { channel: message.channel.clone(), thread_id: message.thread_id.clone() };
        let target = MessageRef {
            channel: message.channel.clone(),
            thread_id: message.thread_id.clone(),
            message_id: message.message_id.clone(),
        };
        for chat in self.listening_chats() {
            if !self.has_subchats(&chat) {
                continue;
            }
            for external_id in self.echo_in_subchat(&chat, &thread, &text).await? {
                self.map_message(&external_id, &target);
            }
        }
        Ok(())
    }

    /// Nella sotto-chat che il thread ha gia' nella `chat` (riaperta se era chiusa). Mai una
    /// nuova: se manca, o e' sparita dal servizio, l'eco non c'e'. Ritorna gli id esterni
    /// delle parti (nessuno se non e' stato inviato nulla).
    async fn echo_in_subchat(&self, chat: &str, thread: &ThreadKey, text: &str) -> Result<Vec<String>, ConnectorError> {
        let mapped = self.state.lock().subchats.subchat_of(thread, chat);
        let Some(subchat) = mapped else { return Ok(Vec::new()) };
        let place = SubchatKey { chat: chat.to_string(), subchat: subchat.clone() };
        if !self.reopen_if_closed(&place).await? {
            return Ok(Vec::new());
        }
        let out = OutboundMessage {
            chat: chat.to_string(),
            subchat: Some(subchat),
            text: text.to_string(),
            reply_to_external: None,
        };
        match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
            Ok(external_ids) => Ok(external_ids),
            Err(ConnectorError::SubchatGone) => {
                log::info!("connettori {}: sotto-chat sparita nel servizio, eco non inviata", self.instance());
                self.forget_subchat(&place);
                Ok(Vec::new())
            }
            Err(e) if e.stops_the_supervisor() => Err(e),
            Err(e) => {
                log::warn!("connettori {}: eco non inviata ({})", self.instance(), e.class());
                Ok(Vec::new())
            }
        }
    }
}
