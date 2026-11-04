//! Segreti dell'app nel portachiavi del sistema operativo (Credential Manager su
//! Windows, Keychain su macOS, Secret Service su Linux, tramite il crate
//! `keyring`). Il resto dell'app vede solo il trait `SecretStore`: i test usano
//! un finto in memoria e un altro backend non tocca chi lo usa.
//!
//! Un segreto non finisce mai in un messaggio d'errore ne' nel log: gli errori
//! descrivono solo il tipo di guasto.

/// Servizio sotto cui l'app registra i suoi segreti nel portachiavi.
const SERVICE: &str = "cli-companion";

/// Archivio di segreti, indicizzati per nome del conto.
pub trait SecretStore: Send + Sync {
    /// Il segreto salvato, `None` se non c'e'.
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    /// Salva (o sostituisce) il segreto.
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    /// Toglie il segreto; nessun errore se non c'era.
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// Il portachiavi del sistema operativo.
pub struct OsKeyring;

impl OsKeyring {
    fn entry(account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, account).map_err(|e| describe(&e))
    }
}

impl SecretStore for OsKeyring {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match Self::entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(describe(&e)),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        Self::entry(account)?.set_password(secret).map_err(|e| describe(&e))
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(describe(&e)),
        }
    }
}

/// Descrizione di un errore del portachiavi senza il suo contenuto: alcune
/// varianti portano i byte letti, cioe' il segreto stesso.
fn describe(e: &keyring::Error) -> String {
    use keyring::Error as E;
    let detail = match e {
        E::PlatformFailure(inner) => format!("errore del sistema ({inner})"),
        E::NoStorageAccess(inner) => format!("accesso negato ({inner})"),
        E::NoDefaultStore => "portachiavi non disponibile su questo sistema".to_string(),
        E::BadEncoding(_) | E::BadDataFormat(..) => "contenuto salvato non leggibile".to_string(),
        E::TooLong(attr, limit) => format!("{attr} oltre il limite di {limit}"),
        _ => "errore inatteso".to_string(),
    };
    format!("portachiavi del sistema: {detail}")
}

/// Archivio in memoria per i test: nessun accesso al portachiavi vero.
#[cfg(test)]
#[derive(Default)]
pub struct MemorySecrets(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl SecretStore for MemorySecrets {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_never_carry_the_secret_bytes() {
        let secret = b"sk-very-secret".to_vec();
        let msg = describe(&keyring::Error::BadEncoding(secret));
        assert!(!msg.contains("sk-very-secret"), "{msg}");
        assert!(!msg.contains("115"), "{msg}");
        assert!(msg.starts_with("portachiavi del sistema"));
    }

    #[test]
    fn the_memory_store_behaves_like_a_keyring() {
        let store = MemorySecrets::default();
        assert_eq!(store.get("a").unwrap(), None);
        store.set("a", "x").unwrap();
        store.set("a", "y").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("y"));
        store.delete("a").unwrap();
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
    }
}
