//! Le notifiche dell'HUB verso i canali di contatto. Un solo `NoticeSink`
//! (`NoticeRouter`, registrato una volta in `setup` con `hub::set_notice_sink`)
//! inoltra ogni `ContactNotice` ai supervisori di contatto in esecuzione la cui
//! istanza dichiara `receive_notices` e ha l'interruttore `notices` acceso.
//! L'inoltro passa da un canale LIMITATO con `try_send`: se e' pieno la notifica
//! si scarta e si registra nel log, senza mai bloccare l'HUB (che chiama `deliver`
//! da un thread del suo pool).
//!
//! Qui si guarda solo l'interruttore generale `notices.all`: il filtro per tipo
//! lo applica il connettore (`ContactConnector::wants_notice`) quando la notifica
//! arriva al supervisore (docs/CONNECTORS.MD §10).

use crate::connectors::descriptor::{ConnectorDescriptor, HubCapability};
use crate::connectors::runtime::store::ConnectorsStore;
use crate::hub::{ContactNotice, NoticeSink};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::mpsc::{self, error::TrySendError};

/// Notifiche in attesa per istanza.
pub const NOTICE_QUEUE: usize = 32;

pub struct NoticeRouter {
    store: Arc<ConnectorsStore>,
    targets: Mutex<HashMap<String, mpsc::Sender<ContactNotice>>>,
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
    pub fn register(&self, instance: &str, descriptor: &ConnectorDescriptor) -> Option<mpsc::Receiver<ContactNotice>> {
        if !accepts_notices(descriptor) {
            return None;
        }
        let (tx, rx) = mpsc::channel(NOTICE_QUEUE);
        self.lock().insert(instance.to_string(), tx);
        Some(rx)
    }

    /// Il supervisore si e' fermato: via il suo canale (le notifiche in coda si perdono).
    pub fn unregister(&self, instance: &str) {
        self.lock().remove(instance);
    }

    /// Via tutti i canali (HUB spento).
    pub fn clear(&self) {
        self.lock().clear();
    }

    #[cfg(test)]
    pub fn registered(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, mpsc::Sender<ContactNotice>>> {
        self.targets.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl NoticeSink for NoticeRouter {
    fn deliver(&self, notice: &ContactNotice) {
        // Copia dei mittenti: nessun lock tenuto durante l'inoltro.
        let targets: Vec<(String, mpsc::Sender<ContactNotice>)> =
            self.lock().iter().map(|(id, tx)| (id.clone(), tx.clone())).collect();
        for (instance, tx) in targets {
            let allowed = self.store.read(|file| file.instance(&instance).is_some_and(|i| i.enabled && i.notices.all));
            if !allowed {
                continue;
            }
            match tx.try_send(notice.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    log::warn!("connettori {instance}: coda delle notifiche piena, notifica scartata");
                }
                Err(TrySendError::Closed(_)) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
