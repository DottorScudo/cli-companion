//! L'abbinamento di un account e i collegamenti nel file: il codice presentato,
//! la creazione del collegamento e le modifiche ai collegamenti esistenti.

use super::replies;
use super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, InboundMessage, PairingOutcome};
use crate::connectors::ids::LINK_PREFIX;
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::model::{ConnectorLink, DISPLAY_NAME_MAX};
use crate::connectors::runtime::pairing::PairAttempt;
use crate::connectors::runtime::store::update_blocking;
use crate::connectors::runtime::time::with_timeout;

/// Il nome mostrato per un account: il nome che il servizio dichiara, senza
/// caratteri di controllo e corto; se e' vuoto, l'id esterno nello stesso modo.
pub(super) fn display_name(sender_name: &str, account: &str) -> String {
    let clean = |raw: &str| -> String {
        let text: String = raw.chars().filter(|c| !c.is_control()).collect();
        text.trim().chars().take(DISPLAY_NAME_MAX).collect()
    };
    let name = clean(sender_name);
    if name.is_empty() {
        clean(account)
    } else {
        name
    }
}

impl ContactCtx<'_> {
    /// Il collegamento di un account di questa istanza.
    pub(super) fn link_of(&self, account: &str) -> Option<ConnectorLink> {
        self.store.read(|f| f.links_of(self.instance()).find(|l| l.external_account == account).cloned())
    }

    /// Applica `change` al collegamento `link_id` e lo salva. `false` (e un avviso nel log)
    /// se il salvataggio non riesce: chi chiama decide cosa dire all'utente.
    pub(super) async fn change_link(
        &self,
        link_id: &str,
        change: impl FnOnce(&mut ConnectorLink) + Send + 'static,
    ) -> bool {
        let id = link_id.to_string();
        let saved = update_blocking(self.store, move |file| {
            if let Some(link) = file.links.iter_mut().find(|l| l.id == id) {
                change(link);
            }
            Ok(())
        })
        .await;
        match saved {
            Ok(_) => true,
            Err(e) => {
                log::warn!("connettori {}: collegamento non salvato: {e}", self.instance());
                false
            }
        }
    }

    /// Toglie il collegamento dal file (e la memoria dell'account).
    pub(super) async fn remove_link(&self, link: &ConnectorLink) {
        let id = link.id.clone();
        let removed = update_blocking(self.store, move |file| {
            file.links.retain(|l| l.id != id);
            Ok(())
        })
        .await;
        if let Err(e) = removed {
            log::warn!("connettori {}: collegamento non rimosso: {e}", self.instance());
        }
        self.session.forget(&link.external_account);
    }

    /// Un codice di abbinamento: se giusto crea il collegamento; il connettore risponde
    /// come crede e, a collegamento fatto, il runtime manda l'aiuto.
    pub(super) async fn pair(&self, message: &InboundMessage, code: &str) -> Result<(), ConnectorError> {
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
        if matches!(outcome, PairingOutcome::Linked { .. }) {
            self.reply(&message.chat, &replies::help()).await?;
        }
        Ok(())
    }

    /// Crea il collegamento (uno solo per account) rispettando il limite per istanza.
    async fn create_link(&self, message: &InboundMessage) -> Result<String, String> {
        let now = self.clock.now_ms();
        let link = ConnectorLink {
            id: self.ids.next(LINK_PREFIX, now),
            instance: self.instance().to_string(),
            external_account: message.account.clone(),
            external_chat: message.chat.clone(),
            display_name: display_name(&message.sender_name, &message.account),
            current: None,
            channel: None,
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
}
