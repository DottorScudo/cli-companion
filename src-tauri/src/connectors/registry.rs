//! Il registro dei connettori: l'unico punto del core che li nomina (insieme a
//! `connectors.rs` per dichiarare i moduli). Un connettore nuovo e' una riga in
//! `CONNECTORS`. Quali istanze esistono e se sono abilitate lo decide l'utente
//! (`connectors.json`); il registro dice solo cosa e' compilato nell'app.
//! Nei test un thread puo' sostituirlo con `test_registry`.

use super::contracts::ConnectorImpl;
use super::{gmail, telegram};

#[cfg(test)]
pub(crate) mod test_registry;

/// I connettori compilati nell'app. Per aggiungerne uno: una riga qui (e una nel test
/// architetturale).
static CONNECTORS: &[ConnectorImpl] =
    &[ConnectorImpl::Source(&gmail::GMAIL), ConnectorImpl::Contact(&telegram::TELEGRAM)];

/// Tutti i connettori registrati.
pub fn all() -> &'static [ConnectorImpl] {
    #[cfg(test)]
    if let Some(registered) = test_registry::registered() {
        return registered;
    }
    CONNECTORS
}

/// Il connettore con questo id, se e' registrato.
pub fn find(id: &str) -> Option<ConnectorImpl> {
    all().iter().copied().find(|c| c.descriptor().id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::testing;

    #[test]
    fn production_registry_has_only_the_real_connectors() {
        assert_eq!(all().len(), 2);
        assert_eq!(find("gmail").map(|c| c.descriptor().id), Some("gmail"));
        assert_eq!(find("telegram").map(|c| c.descriptor().id), Some("telegram"));
        assert!(find("fake-source").is_none());
    }

    #[test]
    fn the_test_override_is_per_thread_and_restored_on_drop() {
        {
            let _registered = test_registry::Registered::new(testing::fakes());
            assert_eq!(all().len(), 2);
            assert_eq!(find("fake-source").map(|c| c.descriptor().id), Some("fake-source"));
            assert!(find("fake-contact").is_some() && find("nope").is_none() && find("telegram").is_none());
            // Un altro thread vede il registro di produzione.
            assert!(std::thread::spawn(|| find("telegram").is_some() && find("fake-source").is_none()).join().unwrap());
        }
        assert!(find("telegram").is_some() && find("fake-source").is_none());
    }
}
