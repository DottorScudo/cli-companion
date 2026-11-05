//! Una notifica dell'HUB verso i collegamenti: il testo, a chi va e la mappa che
//! lega ogni parte inviata al messaggio dell'HUB (una risposta alla notifica
//! torna cosi' nel suo thread).

use super::refs::MessageRef;
use super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, OutboundMessage};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::model::ConnectorLink;
use crate::connectors::runtime::time::with_timeout;
use crate::hub::ContactNotice;

/// `"<canale> › <thread>"` e sotto il testo completo del messaggio (o, se manca,
/// il titolo della notifica). Puro.
pub(super) fn notice_text(notice: &ContactNotice) -> String {
    let body = notice.text.trim();
    let body = if body.is_empty() { notice.notice.title.trim() } else { body };
    format!("{}\n{body}", super::replies::thread_label(&notice.channel_name, &notice.thread_title))
}

/// Il collegamento riceve notifiche all'istante `now`?
fn is_listening(link: &ConnectorLink, now: i64) -> bool {
    link.muted_until.is_none_or(|until| until <= now)
}

impl ContactCtx<'_> {
    /// Una notifica dell'HUB a ogni account collegato e non silenziato, se il
    /// connettore ne vuole il tipo; l'id esterno di ogni parte inviata entra nella
    /// mappa. Un errore del servizio che ferma il supervisore (credenziali,
    /// configurazione) risale; gli altri si registrano.
    pub async fn send_notice(&self, notice: &ContactNotice) -> Result<(), ConnectorError> {
        if !self.connector.wants_notice(self.ctx.config(), notice.category) {
            return Ok(());
        }
        let now = self.clock.now_ms();
        let links: Vec<ConnectorLink> =
            self.store.read(|f| f.links_of(self.instance()).filter(|l| is_listening(l, now)).cloned().collect());
        let text = notice_text(notice);
        let target = MessageRef {
            channel: notice.notice.channel.clone(),
            thread_id: notice.notice.thread_id.clone(),
            message_id: notice.notice.message_id.clone(),
        };
        for link in links {
            let out = OutboundMessage { chat: link.external_chat, text: text.clone(), reply_to_external: None };
            match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
                Ok(external_ids) => external_ids.iter().for_each(|id| self.map_message(id, &target)),
                Err(e) if e.stops_the_supervisor() => return Err(e),
                Err(e) => log::warn!("connettori {}: notifica non inviata ({})", self.instance(), e.class()),
            }
        }
        Ok(())
    }
}
