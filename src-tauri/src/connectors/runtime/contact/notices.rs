//! Una notifica dell'HUB verso i collegamenti: il testo, a chi va e la mappa che
//! lega ogni parte inviata al messaggio dell'HUB (una risposta alla notifica
//! torna cosi' nel suo thread). In una chat con le sotto-chat la notifica va in
//! quella del suo thread, aperta al primo bisogno.

use super::refs::MessageRef;
use super::replies::thread_label;
use super::subchats::{SubchatSend, ThreadKey};
use super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, OutboundMessage};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::model::ConnectorLink;
use crate::connectors::runtime::time::with_timeout;
use crate::hub::ContactNotice;
use std::collections::HashSet;

/// Il testo completo del messaggio o, se manca, il titolo della notifica: il testo
/// di una notifica nella sotto-chat del thread, che ha gia' il nome del thread. Puro.
pub(super) fn notice_body(notice: &ContactNotice) -> &str {
    let body = notice.text.trim();
    if body.is_empty() {
        notice.notice.title.trim()
    } else {
        body
    }
}

/// `"<canale> › <thread>"` e sotto `notice_body`: il testo di una notifica nella chat
/// principale. Puro.
pub(super) fn notice_text(notice: &ContactNotice) -> String {
    format!("{}\n{}", subchat_title(notice), notice_body(notice))
}

/// Il nome della sotto-chat del thread della notifica.
fn subchat_title(notice: &ContactNotice) -> String {
    thread_label(&notice.channel_name, &notice.thread_title)
}

/// Il collegamento riceve notifiche all'istante `now`?
fn is_listening(link: &ConnectorLink, now: i64) -> bool {
    link.muted_until.is_none_or(|until| until <= now)
}

impl ContactCtx<'_> {
    /// Una notifica dell'HUB a ogni chat dei collegamenti non silenziati (una volta per
    /// chat), se il connettore ne vuole il tipo; l'id esterno di ogni parte inviata entra
    /// nella mappa. Un errore del servizio che ferma il supervisore (credenziali,
    /// configurazione) risale; gli altri si registrano.
    pub async fn send_notice(&self, notice: &ContactNotice) -> Result<(), ConnectorError> {
        if !self.connector.wants_notice(self.ctx.config(), notice.category) {
            return Ok(());
        }
        let now = self.clock.now_ms();
        let links: Vec<ConnectorLink> =
            self.store.read(|f| f.links_of(self.instance()).filter(|l| is_listening(l, now)).cloned().collect());
        let target = MessageRef {
            channel: notice.notice.channel.clone(),
            thread_id: notice.notice.thread_id.clone(),
            message_id: notice.notice.message_id.clone(),
        };
        let mut served: HashSet<String> = HashSet::new();
        for link in links {
            if !served.insert(link.external_chat.clone()) {
                continue;
            }
            let external_ids = if self.has_subchats(&link.external_chat) {
                self.notice_in_subchat(&link.external_chat, notice).await?
            } else {
                self.notice_in_chat(&link.external_chat, notice_text(notice)).await?
            };
            external_ids.iter().for_each(|id| self.map_message(id, &target));
        }
        Ok(())
    }

    /// Nella sotto-chat del thread, senza il prefisso col nome del thread; se la
    /// sotto-chat non si apre, nella chat principale col prefisso.
    async fn notice_in_subchat(&self, chat: &str, notice: &ContactNotice) -> Result<Vec<String>, ConnectorError> {
        let thread = ThreadKey { channel: notice.notice.channel.clone(), thread_id: notice.notice.thread_id.clone() };
        match self.send_in_subchat(chat, &thread, &subchat_title(notice), notice_body(notice)).await? {
            SubchatSend::Sent(external_ids) => Ok(external_ids),
            SubchatSend::Failed => Ok(Vec::new()),
            SubchatSend::Unavailable => {
                log::info!(
                    "connettori {}: notifica nella chat principale, sotto-chat non disponibile",
                    self.instance()
                );
                self.notice_in_chat(chat, notice_text(notice)).await
            }
        }
    }

    /// Nella chat principale. Ritorna gli id esterni delle parti (nessuno se l'invio fallisce).
    async fn notice_in_chat(&self, chat: &str, text: String) -> Result<Vec<String>, ConnectorError> {
        let out = OutboundMessage { chat: chat.to_string(), subchat: None, text, reply_to_external: None };
        match with_timeout(CALL_TIMEOUT, self.connector.send(self.ctx, &out)).await {
            Ok(external_ids) => Ok(external_ids),
            Err(e) if e.stops_the_supervisor() => Err(e),
            Err(e) => {
                log::warn!("connettori {}: notifica non inviata ({})", self.instance(), e.class());
                Ok(Vec::new())
            }
        }
    }
}
