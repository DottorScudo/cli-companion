//! Le notifiche dell'HUB e gli eventi sui suoi thread verso i canali di contatto. Un
//! solo `NoticeSink` (`NoticeRouter`, registrato una volta in `setup` con
//! `hub::set_notice_sink`) inoltra ogni `ContactNotice` ai supervisori di contatto in
//! esecuzione la cui istanza dichiara `receive_notices` e ha l'interruttore `notices`
//! acceso. Gli eventi di ciclo di vita (thread eliminato, workspace rimosso) passano
//! dalla stessa coda, cosi' un supervisore li tratta nell'ordine d'arrivo, ma ignorano
//! l'interruttore delle notifiche: le sotto-chat si allineano ai thread comunque.
//! L'inoltro passa da un canale LIMITATO con `try_send`: se e' pieno l'evento si scarta
//! e si registra nel log, senza mai bloccare l'HUB (che chiama da un thread del suo pool).
//!
//! Qui si guarda solo l'interruttore generale `notices.all`: il filtro per tipo
//! lo applica il connettore (`ContactConnector::wants_notice`) quando la notifica
//! arriva al supervisore (docs/CONNECTORS.MD §10).

use crate::connectors::descriptor::{ConnectorDescriptor, HubCapability};
use crate::connectors::model::ConnectorInstance;
use crate::connectors::runtime::contact::subchats::ThreadKey;
use crate::connectors::runtime::store::ConnectorsStore;
use crate::hub::{Channel, ContactNotice, NoticeSink};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::mpsc::{self, error::TrySendError};

/// Eventi in attesa per istanza.
pub const NOTICE_QUEUE: usize = 32;

/// Cio' che l'HUB comunica a un supervisore di contatto.
#[derive(Debug, Clone)]
pub enum ContactEvent {
    /// Una notifica da mandare ai collegamenti.
    Notice(ContactNotice),
    /// Un thread e' stato eliminato.
    ThreadRemoved(ThreadKey),
    /// Un workspace e' stato rimosso, con tutti i suoi thread.
    ChannelRemoved(Channel),
}

pub struct NoticeRouter {
    store: Arc<ConnectorsStore>,
    targets: Mutex<HashMap<String, mpsc::Sender<ContactEvent>>>,
}

/// Il descrittore dichiara di poter ricevere le notifiche?
pub fn accepts_notices(descriptor: &ConnectorDescriptor) -> bool {
    descriptor.has_capability(HubCapability::ReceiveNotices)
}

impl NoticeRouter {
    pub fn new(store: Arc<ConnectorsStore>) -> Self {
        Self { store, targets: Mutex::new(HashMap::new()) }
    }

    /// Registra un supervisore di contatto: ritorna il ricevitore da cui legge.
    /// `None` se il connettore non dichiara `receive_notices`.
    pub fn register(&self, instance: &str, descriptor: &ConnectorDescriptor) -> Option<mpsc::Receiver<ContactEvent>> {
        if !accepts_notices(descriptor) {
            return None;
        }
        let (tx, rx) = mpsc::channel(NOTICE_QUEUE);
        self.lock().insert(instance.to_string(), tx);
        Some(rx)
    }

    /// Il supervisore si e' fermato: via il suo canale (gli eventi in coda si perdono).
    pub fn unregister(&self, instance: &str) {
        self.lock().remove(instance);
    }

    /// Via tutti i canali (HUB spento).
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Il thread e' stato eliminato: lo sanno le istanze abilitate in esecuzione.
    pub fn thread_removed(&self, thread: ThreadKey) {
        self.forward(&ContactEvent::ThreadRemoved(thread), |_| true);
    }

    /// Il workspace e' stato rimosso: lo sanno le istanze abilitate in esecuzione.
    pub fn channel_removed(&self, channel: Channel) {
        self.forward(&ContactEvent::ChannelRemoved(channel), |_| true);
    }

    #[cfg(test)]
    pub fn registered(&self) -> usize {
        self.lock().len()
    }

    /// Mette `event` nella coda delle istanze abilitate che `wanted` accetta.
    fn forward(&self, event: &ContactEvent, wanted: impl Fn(&ConnectorInstance) -> bool) {
        // Copia dei mittenti: nessun lock tenuto durante l'inoltro.
        let targets: Vec<(String, mpsc::Sender<ContactEvent>)> =
            self.lock().iter().map(|(id, tx)| (id.clone(), tx.clone())).collect();
        for (instance, tx) in targets {
            let allowed = self.store.read(|file| file.instance(&instance).is_some_and(|i| i.enabled && wanted(i)));
            if !allowed {
                continue;
            }
            match tx.try_send(event.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    log::warn!("connettori {instance}: coda degli eventi piena, evento scartato");
                }
                Err(TrySendError::Closed(_)) => {}
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, mpsc::Sender<ContactEvent>>> {
        self.targets.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl NoticeSink for NoticeRouter {
    fn deliver(&self, notice: &ContactNotice) {
        self.forward(&ContactEvent::Notice(notice.clone()), |instance| instance.notices.all);
    }
}

#[cfg(test)]
mod tests;
