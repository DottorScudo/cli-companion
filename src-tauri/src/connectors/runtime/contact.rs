//! La logica di un canale di contatto (docs/CONNECTORS.MD §10), senza cicli ne'
//! tempo: un messaggio in entrata (dedupe, abbinamento, scrittura nel thread
//! corrente dell'account collegato) e una notifica in uscita (a ogni account
//! collegato non silenziato). Niente `AppHandle`: tutto passa da `HubPort`.
//!
//! Gli account non collegati si ignorano in silenzio (nessuna risposta: non si
//! rivela che il bot esiste); solo un codice di abbinamento riceve risposta.

use crate::connectors::contracts::{
    Clock, ConnectorContext, ConnectorError, ContactConnector, HubPort, InboundMessage, OutboundMessage,
    PairingOutcome, StateHandle,
};
use crate::connectors::ids::{IdGenerator, LINK_PREFIX};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::model::{ConnectorLink, DISPLAY_NAME_MAX};
use crate::connectors::runtime::pairing::{PairAttempt, PairingRegistry};
use crate::connectors::runtime::store::{update_blocking, ConnectorsStore};
use crate::connectors::runtime::time::with_timeout;
use crate::hub::send::UserMessage;
use crate::hub::{HubNotice, MessageOrigin};
use std::sync::Arc;

/// Lo "slot" della dedupe dei messaggi in entrata (le regole delle fonti hanno il loro id).
const INBOUND_SEEN: &str = "inbound";

pub struct ContactCtx<'a> {
    pub connector: &'a dyn ContactConnector,
    pub ctx: &'a ConnectorContext,
    pub hub: &'a dyn HubPort,
    pub state: &'a StateHandle,
    pub store: &'a Arc<ConnectorsStore>,
    pub pairing: &'a PairingRegistry,
    pub ids: &'a IdGenerator,
    pub clock: &'a dyn Clock,
}

/// Il nome mostrato per un account: l'id esterno senza caratteri di controllo, corto.
fn display_name(account: &str) -> String {
    account.chars().filter(|c| !c.is_control()).take(DISPLAY_NAME_MAX).collect()
}

impl ContactCtx<'_> {
    fn instance(&self) -> &str {
        self.ctx.instance()
    }

    fn mark_seen(&self, external_id: &str) {
        self.state.lock().mark_seen(INBOUND_SEEN, external_id);
    }

    /// Un messaggio in entrata. Un errore ferma il lotto prima del cursore: il messaggio si ripresenta.
    pub async fn handle_inbound(&self, message: InboundMessage) -> Result<(), ConnectorError> {
        if self.state.lock().is_seen(INBOUND_SEEN, &message.external_id) {
            return Ok(());
        }
        if let Some(code) = message.pairing_code.as_deref() {
            self.pair(&message, code).await;
        } else {
            self.forward(&message).await?;
        }
        self.mark_seen(&message.external_id);
        Ok(())
    }

    async fn pair(&self, message: &InboundMessage, code: &str) {
        let outcome = match self.pairing.attempt(self.instance(), code, self.clock.now_ms()) {
            PairAttempt::Accepted => match self.create_link(message).await {
                Ok(display_name) => PairingOutcome::Linked { display_name },
                Err(reason) => {
                    log::warn!("connettori {}: abbinamento non riuscito: {reason}", self.instance());
                    PairingOutcome::Rejected
                }
            },
            PairAttempt::Rejected => PairingOutcome::Rejected,
            PairAttempt::TooManyAttempts => PairingOutcome::TooManyAttempts,
        };
        let reply = self.connector.pairing_reply(self.ctx, &outcome, &message.chat);
        if let Err(e) = with_timeout(CALL_TIMEOUT, reply).await {
            log::warn!("connettori {}: risposta di abbinamento non inviata ({})", self.instance(), e.class());
        }
    }

    /// Crea il collegamento (uno solo per account) rispettando il limite per istanza.
    async fn create_link(&self, message: &InboundMessage) -> Result<String, String> {
        let now = self.clock.now_ms();
        let link = ConnectorLink {
            id: self.ids.next(LINK_PREFIX, now),
            instance: self.instance().to_string(),
            external_account: message.account.clone(),
            external_chat: message.chat.clone(),
            display_name: display_name(&message.account),
            current: None,
            muted_until: None,
            created_at: now,
        };
        link.check()?;
        let shown = link.display_name.clone();
        update_blocking(self.store, move |file| {
            let known = file.links_of(&link.instance).any(|l| l.external_account == link.external_account);
            if known {
                return Ok(());
            }
            file.check_link_capacity(&link.instance)?;
            file.links.push(link);
            Ok(())
        })
        .await?;
        Ok(shown)
    }

    /// Il messaggio di un account collegato va al thread corrente del collegamento.
    async fn forward(&self, message: &InboundMessage) -> Result<(), ConnectorError> {
        let account = message.account.as_str();
        let link = self.store.read(|f| f.links_of(self.instance()).find(|l| l.external_account == account).cloned());
        let Some(link) = link else { return Ok(()) };
        let Some(current) = link.current.clone() else {
            log::info!("connettori {}: messaggio ignorato, il collegamento non ha un thread corrente", self.instance());
            return Ok(());
        };
        let exists = self.hub.thread_exists(&current.channel, &current.thread_id).await;
        if !exists.map_err(ConnectorError::Transient)? {
            log::info!("connettori {}: thread corrente sparito, collegamento azzerato", self.instance());
            let id = link.id.clone();
            let cleared = update_blocking(self.store, move |file| {
                file.links.iter_mut().filter(|l| l.id == id).for_each(|l| l.current = None);
                Ok(())
            });
            return cleared.await.map(|_| ()).map_err(ConnectorError::Transient);
        }
        if message.files.is_empty() && message.text.trim().is_empty() {
            return Ok(());
        }
        let reply_to = message
            .reply_to_external
            .as_deref()
            .and_then(|external| self.state.lock().external_to_hub.get(external).map(str::to_string));
        let origin = MessageOrigin {
            connector: self.ctx.descriptor().id.to_string(),
            instance: self.instance().to_string(),
            external_id: message.external_id.clone(),
        };
        let build = |reply_to: Option<String>| UserMessage {
            channel: current.channel.clone(),
            thread_id: current.thread_id.clone(),
            body: message.text.clone(),
            reply_to,
            attachment_ids: Vec::new(),
            origin: Some(origin.clone()),
        };
        let mut sent = self.send(build(reply_to.clone()), message).await;
        if sent.is_err() && reply_to.is_some() {
            // Il messaggio citato puo' essere di un altro thread: si riprova senza citazione.
            sent = self.send(build(None), message).await;
        }
        let sent = sent.map_err(ConnectorError::Transient)?;
        self.state.lock().link_messages(&message.external_id, &sent.id);
        Ok(())
    }

    async fn send(&self, user: UserMessage, message: &InboundMessage) -> Result<crate::hub::Message, String> {
        if message.files.is_empty() {
            self.hub.send_user_message(user).await
        } else {
            self.hub.send_user_files(user, message.files.clone()).await
        }
    }

    /// Una notifica dell'HUB a ogni account collegato e non silenziato; l'id esterno
    /// di ogni messaggio inviato entra nella mappa. Un errore del servizio che
    /// ferma il supervisore (credenziali, configurazione) risale; gli altri si registrano.
    pub async fn send_notice(&self, notice: &HubNotice) -> Result<(), ConnectorError> {
        let now = self.clock.now_ms();
        let links: Vec<ConnectorLink> = self.store.read(|f| {
            f.links_of(self.instance()).filter(|l| l.muted_until.is_none_or(|until| until <= now)).cloned().collect()
        });
        let text = format!("{}\n{}", notice.title, notice.body).trim().to_string();
        for link in links {
            let out = OutboundMessage { chat: link.external_chat, text: text.clone(), reply_to_external: None };
            match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
                Ok(external_id) => self.state.lock().link_messages(&external_id, &notice.message_id),
                Err(e) if e.stops_the_supervisor() => return Err(e),
                Err(e) => log::warn!("connettori {}: notifica non inviata ({})", self.instance(), e.class()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
