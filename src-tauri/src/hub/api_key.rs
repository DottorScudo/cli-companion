//! API key del provider del modello dei segretari, nel portachiavi del sistema
//! (`crate::secrets`). Non finisce mai nei file JSON, nel log ne' nei DTO: il
//! frontend sa solo se c'e' (`api_key_set`). La legge solo il backend.
//!
//! Stato gestito da Tauri. La presenza della chiave si chiede al portachiavi la
//! prima volta e poi resta in memoria, aggiornata da `set` e `clear`: ogni
//! accesso al portachiavi avviene sotto lo stesso lock, quindi memoria e
//! portachiavi non divergono.

use crate::secrets::SecretStore;
use std::sync::Mutex;

/// Nome del conto della chiave nel portachiavi (servizio `cli-companion`).
const ACCOUNT: &str = "hub-provider-api-key";
/// Lunghezza massima della chiave, in caratteri: ampiamente sopra le chiavi
/// reali e sotto il limite del Credential Manager (2560 byte in UTF-16).
pub const API_KEY_MAX: usize = 1024;

pub struct ApiKey {
    store: Box<dyn SecretStore>,
    /// `Some(presente)` dopo la prima lettura o scrittura.
    present: Mutex<Option<bool>>,
}

impl ApiKey {
    pub fn new(store: Box<dyn SecretStore>) -> Self {
        Self { store, present: Mutex::new(None) }
    }

    /// La chiave e' salvata? Dal portachiavi la prima volta, poi dalla memoria.
    pub fn is_set(&self) -> Result<bool, String> {
        let mut present = self.present.lock().unwrap();
        if let Some(known) = *present {
            return Ok(known);
        }
        let known = self.store.get(ACCOUNT)?.is_some();
        *present = Some(known);
        Ok(known)
    }

    /// Valida e salva la chiave (sostituisce quella precedente).
    pub fn set(&self, raw: &str) -> Result<(), String> {
        let key = validate(raw)?;
        let mut present = self.present.lock().unwrap();
        self.store.set(ACCOUNT, key)?;
        *present = Some(true);
        Ok(())
    }

    /// Toglie la chiave; nessun errore se non c'era.
    pub fn clear(&self) -> Result<(), String> {
        let mut present = self.present.lock().unwrap();
        self.store.delete(ACCOUNT)?;
        *present = Some(false);
        Ok(())
    }

    /// La chiave in chiaro, solo per il backend (il client del modello).
    pub fn read(&self) -> Result<Option<String>, String> {
        let mut present = self.present.lock().unwrap();
        let key = self.store.get(ACCOUNT)?;
        *present = Some(key.is_some());
        Ok(key)
    }
}

/// La chiave finisce in un'intestazione HTTP: trim, 1..=`API_KEY_MAX`
/// caratteri ASCII visibili (niente spazi ne' caratteri di controllo). Il
/// messaggio d'errore non cita mai la chiave.
fn validate(raw: &str) -> Result<&str, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Err("la API key è vuota: per toglierla usa «Rimuovi»".to_string());
    }
    if key.len() > API_KEY_MAX {
        return Err(format!("la API key supera i {API_KEY_MAX} caratteri"));
    }
    if !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("la API key contiene spazi o caratteri non validi".to_string());
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecrets;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Conta le letture e puo' fallire: verifica cache ed errori.
    #[derive(Default)]
    struct Probe {
        inner: MemorySecrets,
        reads: AtomicUsize,
        broken: bool,
    }

    impl SecretStore for Arc<Probe> {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.broken {
                return Err("portachiavi del sistema: accesso negato".to_string());
            }
            self.inner.get(account)
        }

        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            if self.broken {
                return Err("portachiavi del sistema: accesso negato".to_string());
            }
            self.inner.set(account, secret)
        }

        fn delete(&self, account: &str) -> Result<(), String> {
            if self.broken {
                return Err("portachiavi del sistema: accesso negato".to_string());
            }
            self.inner.delete(account)
        }
    }

    fn key_with(probe: &Arc<Probe>) -> ApiKey {
        ApiKey::new(Box::new(Arc::clone(probe)))
    }

    #[test]
    fn set_read_and_clear() {
        let key = ApiKey::new(Box::new(MemorySecrets::default()));
        assert!(!key.is_set().unwrap());
        assert_eq!(key.read().unwrap(), None);
        key.set("  sk-abc123  ").unwrap();
        assert!(key.is_set().unwrap());
        assert_eq!(key.read().unwrap().as_deref(), Some("sk-abc123"));
        key.clear().unwrap();
        assert!(!key.is_set().unwrap());
        assert_eq!(key.read().unwrap(), None);
        key.clear().unwrap();
    }

    #[test]
    fn presence_is_read_once_then_cached() {
        let probe = Arc::new(Probe::default());
        probe.inner.set(ACCOUNT, "sk-old").unwrap();
        let key = key_with(&probe);
        assert!(key.is_set().unwrap());
        assert!(key.is_set().unwrap());
        assert_eq!(probe.reads.load(Ordering::SeqCst), 1);
        key.clear().unwrap();
        assert!(!key.is_set().unwrap());
        assert_eq!(probe.reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn keyring_failures_propagate_and_are_not_cached() {
        let probe = Arc::new(Probe { broken: true, ..Probe::default() });
        let key = key_with(&probe);
        assert!(key.is_set().is_err());
        assert!(key.is_set().is_err());
        assert_eq!(probe.reads.load(Ordering::SeqCst), 2);
        assert!(key.set("sk-x").is_err());
        assert!(key.clear().is_err());
    }

    #[test]
    fn invalid_keys_are_rejected_without_quoting_them() {
        let key = ApiKey::new(Box::new(MemorySecrets::default()));
        for bad in ["", "   ", "sk abc", "sk-\nabc", "sk-\u{0}", "sk-é"] {
            let err = key.set(bad).unwrap_err();
            assert!(bad.trim().is_empty() || !err.contains(bad.trim()), "{err}");
        }
        assert!(key.set(&"a".repeat(API_KEY_MAX)).is_ok());
        assert!(key.set(&"a".repeat(API_KEY_MAX + 1)).is_err());
        assert_eq!(key.read().unwrap().map(|k| k.len()), Some(API_KEY_MAX));
    }
}
