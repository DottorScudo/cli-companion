//! La logica di un canale di contatto (docs/CONNECTORS.MD §10), senza cicli ne'
//! tempo: un messaggio in entrata (dedupe, abbinamento, comandi, scrittura nel
//! thread dell'account collegato) e una notifica in uscita (a ogni account
//! collegato non silenziato). Niente `AppHandle`: tutto passa da `HubPort`.
//!
//! Gli account non collegati si ignorano in silenzio (nessuna risposta: non si
//! rivela che il bot esiste); solo un codice di abbinamento riceve risposta.
//! Un file per responsabilita':
//! - `refs`: il riferimento a un messaggio dell'HUB dietro un messaggio esterno (puro);
//! - `session`: la memoria in RAM di ogni account (elenchi, limite dei messaggi, puro);
//! - `replies`: i testi del bot (puri);
//! - `linking`: abbinamento e collegamenti nel file;
//! - `commands`: l'esecuzione dei comandi;
//! - `inbound`: un messaggio normale verso il thread giusto, con gli allegati;
//! - `notices`: le notifiche dell'HUB verso i collegamenti;
//! - `mirror`: l'eco dei messaggi scritti nell'app nella sotto-chat del thread;
//! - `subchats`: le sotto-chat di una chat collegata, una per thread (mappa pura e uso).

mod commands;
mod inbound;
mod linking;
mod mirror;
mod notices;
pub mod refs;
mod replies;
mod session;
pub mod subchats;

use crate::connectors::contracts::{
    Clock, ConnectorContext, ConnectorError, ContactConnector, HubPort, InboundMessage, OutboundMessage, StateHandle,
};
use crate::connectors::ids::IdGenerator;
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::model::ConnectorLink;
use crate::connectors::runtime::pairing::PairingRegistry;
use crate::connectors::runtime::store::ConnectorsStore;
use crate::connectors::runtime::time::with_timeout;
use refs::MessageRef;
use session::Admission;
pub use session::ContactSession;
use std::sync::Arc;
use subchats::valid_subchat_id;

/// Lo "slot" della dedupe dei messaggi in entrata (le regole delle fonti hanno il loro id).
const INBOUND_SEEN: &str = "inbound";

/// Dove scrivere una risposta del bot: la chat e, se c'e', la sua sotto-chat.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    chat: String,
    subchat: Option<String>,
}

impl Place {
    /// La chat principale.
    fn main(chat: &str) -> Self {
        Self { chat: chat.to_string(), subchat: None }
    }
}

pub struct ContactCtx<'a> {
    pub connector: &'a dyn ContactConnector,
    pub ctx: &'a ConnectorContext,
    pub hub: &'a dyn HubPort,
    pub state: &'a StateHandle,
    pub store: &'a Arc<ConnectorsStore>,
    pub pairing: &'a PairingRegistry,
    pub ids: &'a IdGenerator,
    pub clock: &'a dyn Clock,
    /// La memoria in RAM degli account di questa istanza.
    pub session: ContactSession,
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
        self.dispatch(&message).await?;
        self.mark_seen(&message.external_id);
        Ok(())
    }

    /// Abbinamento, poi (per un account collegato) limite, chat del collegamento, comando o
    /// messaggio normale. Gli altri account (anche in un gruppo) si ignorano in silenzio.
    async fn dispatch(&self, message: &InboundMessage) -> Result<(), ConnectorError> {
        if let Some(code) = message.pairing_code.as_deref() {
            return self.pair(message, code).await;
        }
        let Some(link) = self.link_of(&message.account) else { return Ok(()) };
        let Some(place) = self.place_of(message) else {
            log::info!("connettori {}: messaggio ignorato, id di sotto-chat non valido", self.instance());
            return Ok(());
        };
        if !self.admit(&link, &place).await? {
            return Ok(());
        }
        if message.chat != link.external_chat {
            log::info!("connettori {}: messaggio ignorato, non viene dalla chat collegata", self.instance());
            return self.reply(&place, &replies::wrong_chat()).await;
        }
        match message.command.clone() {
            Some(command) => self.run_command(&link, &place, command).await,
            None => self.forward(&link, &place, message).await,
        }
    }

    /// Dove rispondere al messaggio: la sua chat e la sua sotto-chat, se la chat ne ha
    /// (altrimenti la sotto-chat si ignora). `None` se l'id della sotto-chat non e' valido.
    fn place_of(&self, message: &InboundMessage) -> Option<Place> {
        let subchat = message.subchat.clone().filter(|_| self.has_subchats(&message.chat));
        if subchat.as_deref().is_some_and(|id| !valid_subchat_id(id)) {
            return None;
        }
        Some(Place { chat: message.chat.clone(), subchat })
    }

    /// Il messaggio rientra nel limite al minuto? Il primo scarto di una finestra riceve una risposta.
    async fn admit(&self, link: &ConnectorLink, place: &Place) -> Result<bool, ConnectorError> {
        let now = self.clock.now_ms();
        match self.session.with(&link.external_account, |s| s.admit(now)) {
            Admission::Allowed => Ok(true),
            Admission::Dropped { first } => {
                log::info!("connettori {}: messaggio scartato, troppi in un minuto", self.instance());
                if first {
                    self.reply(place, &replies::rate_limited()).await?;
                }
                Ok(false)
            }
        }
    }

    /// L'aiuto adatto alla chat: con le sotto-chat spiega anche quelle.
    fn help_for(&self, chat: &str) -> String {
        if self.has_subchats(chat) {
            replies::subchat_help()
        } else {
            replies::help()
        }
    }

    /// Una risposta del bot nella chat (e nella sotto-chat) del messaggio. Un errore che ferma
    /// il supervisore (credenziali, configurazione) risale; gli altri si registrano con la loro classe.
    async fn reply(&self, place: &Place, text: &str) -> Result<(), ConnectorError> {
        let out = OutboundMessage {
            chat: place.chat.clone(),
            subchat: place.subchat.clone(),
            text: text.to_string(),
            reply_to_external: None,
        };
        match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
            Ok(_) => Ok(()),
            Err(e) if e.stops_the_supervisor() => Err(e),
            Err(e) => {
                log::warn!("connettori {}: risposta non inviata ({})", self.instance(), e.class());
                Ok(())
            }
        }
    }

    /// Lega un messaggio esterno a quello dell'HUB che gli sta dietro: l'id esterno
    /// porta al riferimento completo, l'id del messaggio torna all'id esterno.
    fn map_message(&self, external_id: &str, target: &MessageRef) {
        self.state.lock().link_message_ref(external_id, &target.message_id, &target.encode());
    }
}

#[cfg(test)]
mod tests;
