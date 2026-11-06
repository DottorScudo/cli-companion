//! Le sotto-chat seguono i thread dell'HUB (docs/CONNECTORS.MD §10, «Sotto-chat»): un
//! thread eliminato o un workspace rimosso eliminano le loro sotto-chat, un thread
//! concluso chiude la sua, che si riapre al primo messaggio o alla prima notifica
//! successivi. Decide il runtime, il connettore traduce nel servizio. Una sotto-chat gia'
//! sparita dal servizio non e' un errore (la voce esce dalla mappa); un altro errore del
//! servizio si registra e non si riprova da solo: la voce resta com'era.

use super::{SubchatKey, ThreadKey};
use crate::connectors::contracts::ConnectorError;
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::contact::ContactCtx;
use crate::connectors::runtime::time::with_timeout;
use crate::hub::Channel;
use std::future::Future;

/// Come e' andata una chiamata di ciclo di vita su una sotto-chat.
enum Applied {
    Done,
    /// Nel servizio la sotto-chat non c'e' piu'.
    Gone,
    /// Errore del servizio che non ferma il supervisore, gia' nel log.
    Failed,
}

impl ContactCtx<'_> {
    /// Il thread e' stato eliminato: via le sue sotto-chat, in ogni chat collegata.
    pub async fn thread_removed(&self, thread: &ThreadKey) -> Result<(), ConnectorError> {
        self.delete_subchats_where(|t| t == thread).await
    }

    /// Il workspace e' stato rimosso: via le sotto-chat di tutti i suoi thread.
    pub async fn channel_removed(&self, channel: &Channel) -> Result<(), ConnectorError> {
        self.delete_subchats_where(|t| t.channel == *channel).await
    }

    /// Il thread e' concluso: si chiudono le sue sotto-chat ancora aperte.
    pub(in super::super) async fn close_thread_subchats(&self, thread: &ThreadKey) -> Result<(), ConnectorError> {
        let places = self.state.lock().subchats.places_where(|t| t == thread);
        for place in places {
            if self.is_subchat_closed(&place) || !self.has_subchats(&place.chat) {
                continue;
            }
            let call = self.connector.close_subchat(self.ctx, &place.chat, &place.subchat);
            match self.applied("chiusa", call).await? {
                Applied::Done => self.state.lock().subchats.set_closed(&place, true),
                Applied::Gone => self.forget_subchat(&place),
                Applied::Failed => {}
            }
        }
        Ok(())
    }

    /// Prima di scriverci o di rispondere: se la sotto-chat e' chiusa la riapre. `false` se
    /// nel servizio non c'e' piu' (la voce esce dalla mappa). Un errore del servizio non
    /// impedisce di scrivere (un amministratore scrive anche in una sotto-chat chiusa) e
    /// non si riprova: la voce torna aperta comunque.
    pub(in super::super) async fn reopen_if_closed(&self, place: &SubchatKey) -> Result<bool, ConnectorError> {
        if !self.is_subchat_closed(place) {
            return Ok(true);
        }
        if self.has_subchats(&place.chat) {
            let call = self.connector.reopen_subchat(self.ctx, &place.chat, &place.subchat);
            if matches!(self.applied("riaperta", call).await?, Applied::Gone) {
                self.forget_subchat(place);
                return Ok(false);
            }
        }
        self.state.lock().subchats.set_closed(place, false);
        Ok(true)
    }

    /// Elimina nel servizio le sotto-chat dei thread scelti; la voce esce dalla mappa se la
    /// sotto-chat e' eliminata o gia' sparita.
    async fn delete_subchats_where(&self, selected: impl Fn(&ThreadKey) -> bool) -> Result<(), ConnectorError> {
        let places = self.state.lock().subchats.places_where(selected);
        for place in places {
            if !self.has_subchats(&place.chat) {
                self.forget_subchat(&place);
                continue;
            }
            let call = self.connector.delete_subchat(self.ctx, &place.chat, &place.subchat);
            match self.applied("eliminata", call).await? {
                Applied::Done | Applied::Gone => self.forget_subchat(&place),
                Applied::Failed => {}
            }
        }
        Ok(())
    }

    fn is_subchat_closed(&self, place: &SubchatKey) -> bool {
        self.state.lock().subchats.is_closed(place)
    }

    /// Esegue la chiamata entro il tempo e la riduce al suo esito; solo un errore che
    /// ferma il supervisore (credenziali, configurazione) risale. `what` e' il participio
    /// per il log.
    async fn applied(
        &self,
        what: &str,
        call: impl Future<Output = Result<(), ConnectorError>>,
    ) -> Result<Applied, ConnectorError> {
        match with_timeout(CALL_TIMEOUT, call).await {
            Ok(()) => Ok(Applied::Done),
            Err(ConnectorError::SubchatGone) => Ok(Applied::Gone),
            Err(e) if e.stops_the_supervisor() => Err(e),
            Err(e) => {
                log::warn!("connettori {}: sotto-chat non {what} ({})", self.instance(), e.class());
                Ok(Applied::Failed)
            }
        }
    }
}
