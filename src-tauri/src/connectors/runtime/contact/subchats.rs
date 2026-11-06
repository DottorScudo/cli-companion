//! Le sotto-chat di una chat collegata (docs/CONNECTORS.MD §10, «Sotto-chat»): una
//! conversazione separata per ogni thread dell'HUB (es. i topic di un forum). Il
//! connettore dice quali chat le hanno e le apre; il runtime tiene la mappa thread <->
//! sotto-chat nello stato dell'istanza, la apre al primo bisogno e la ricrea una volta
//! se il servizio dice che non c'e' piu'. Un file per responsabilita':
//! - `map`: la mappa limitata, nei due versi (pura);
//! - `lifecycle`: eliminazione, chiusura e riapertura al ciclo di vita dei thread.

mod lifecycle;
mod map;

pub use map::{valid_subchat_id, SubchatKey, SubchatMap, ThreadKey};

use super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, OutboundMessage};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::time::with_timeout;

/// Invii in una sotto-chat per messaggio: il primo e, se la sotto-chat e' sparita, uno
/// in una sotto-chat ricreata.
const SUBCHAT_SEND_ATTEMPTS: usize = 2;

/// Com'e' andato un invio nella sotto-chat di un thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SubchatSend {
    /// Inviato: l'id esterno di ogni parte.
    Sent(Vec<String>),
    /// Nessuna sotto-chat utilizzabile (non aperta, o sparita anche dopo averla ricreata):
    /// chi chiama ripiega sulla chat principale.
    Unavailable,
    /// Non inviato per un errore del servizio, gia' nel log.
    Failed,
}

impl ContactCtx<'_> {
    /// La `chat` ha le sotto-chat?
    pub(super) fn has_subchats(&self, chat: &str) -> bool {
        self.connector.subchats(self.ctx.config(), chat)
    }

    /// Il thread di una sotto-chat, dalla mappa.
    pub(super) fn thread_of_subchat(&self, place: &SubchatKey) -> Option<ThreadKey> {
        self.state.lock().subchats.thread_of(place)
    }

    /// Toglie una sotto-chat dalla mappa (il thread o la sotto-chat non ci sono piu').
    pub(super) fn forget_subchat(&self, place: &SubchatKey) {
        self.state.lock().subchats.remove_subchat(place);
    }

    /// La sotto-chat del thread nella `chat`: quella della mappa (riaperta se era chiusa)
    /// o una nuova, aperta col nome `title`. `None` se il servizio non l'ha aperta per un
    /// errore che non ferma il supervisore (riga nel log); gli altri errori risalgono.
    pub(super) async fn ensure_subchat(
        &self,
        chat: &str,
        thread: &ThreadKey,
        title: &str,
    ) -> Result<Option<String>, ConnectorError> {
        let mapped = self.state.lock().subchats.subchat_of(thread, chat);
        if let Some(found) = mapped {
            let place = SubchatKey { chat: chat.to_string(), subchat: found.clone() };
            if self.reopen_if_closed(&place).await? {
                return Ok(Some(found));
            }
        }
        let subchat = match with_timeout(CALL_TIMEOUT, self.connector.open_subchat(self.ctx, chat, title)).await {
            Ok(subchat) => subchat,
            Err(e) if e.stops_the_supervisor() => return Err(e),
            Err(e) => {
                log::warn!("connettori {}: sotto-chat non aperta ({})", self.instance(), e.class());
                return Ok(None);
            }
        };
        let place = SubchatKey { chat: chat.to_string(), subchat: subchat.clone() };
        if !self.state.lock().subchats.insert(thread.clone(), place) {
            log::warn!("connettori {}: sotto-chat aperta con un id non valido, ignorata", self.instance());
            return Ok(None);
        }
        Ok(Some(subchat))
    }

    /// Manda `text` nella sotto-chat del thread, aprendola se serve; se il servizio dice
    /// che e' sparita, la voce esce dalla mappa e la sotto-chat si ricrea una volta.
    pub(super) async fn send_in_subchat(
        &self,
        chat: &str,
        thread: &ThreadKey,
        title: &str,
        text: &str,
    ) -> Result<SubchatSend, ConnectorError> {
        for _ in 0..SUBCHAT_SEND_ATTEMPTS {
            let Some(subchat) = self.ensure_subchat(chat, thread, title).await? else {
                return Ok(SubchatSend::Unavailable);
            };
            let out = OutboundMessage {
                chat: chat.to_string(),
                subchat: Some(subchat.clone()),
                text: text.to_string(),
                reply_to_external: None,
            };
            match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
                Ok(external_ids) => return Ok(SubchatSend::Sent(external_ids)),
                Err(ConnectorError::SubchatGone) => {
                    log::info!("connettori {}: sotto-chat sparita nel servizio, la ricreo", self.instance());
                    self.forget_subchat(&SubchatKey { chat: chat.to_string(), subchat });
                }
                Err(e) if e.stops_the_supervisor() => return Err(e),
                Err(e) => {
                    log::warn!(
                        "connettori {}: messaggio non inviato nella sotto-chat ({})",
                        self.instance(),
                        e.class()
                    );
                    return Ok(SubchatSend::Failed);
                }
            }
        }
        Ok(SubchatSend::Unavailable)
    }
}
