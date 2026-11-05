//! I segreti di un'istanza nel portachiavi del sistema (`crate::secrets`): un
//! conto per slot, `connector-<istanza>-<slot>`. Non finiscono mai nei file JSON,
//! nel log, nei DTO, negli errori o negli eventi; l'interfaccia vede solo
//! `{ slot, set }`. Gli slot validi sono quelli dichiarati dal descrittore (campi
//! `secret` e slot interni): chi usa questo modulo passa l'elenco.

use super::descriptor::validate::valid_key;
use super::ids::valid_instance_id;
use crate::secrets::SecretStore;
use serde::Serialize;
use std::sync::Arc;
use ts_rs::TS;

/// Lunghezza massima di un segreto: sotto il limite del Credential Manager (2560
/// byte in UTF-16) e ampiamente sopra token e chiavi reali.
pub const SECRET_MAX: usize = 1024;

/// Uno slot come lo vede il frontend: mai il valore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorSecretSlot")]
pub struct SecretSlot {
    pub slot: String,
    pub set: bool,
}

/// I segreti di un'istanza.
#[derive(Clone)]
pub struct InstanceSecrets {
    store: Arc<dyn SecretStore>,
    instance: String,
}

/// Il nome del conto nel portachiavi; `None` se istanza o slot non sono ben formati.
pub fn account(instance: &str, slot: &str) -> Option<String> {
    (valid_instance_id(instance) && valid_key(slot)).then(|| format!("connector-{instance}-{slot}"))
}

/// Il segreto: trim, 1..=`SECRET_MAX` caratteri ASCII visibili. Il messaggio
/// d'errore non lo cita mai.
pub fn validate_value(raw: &str) -> Result<&str, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("il valore è vuoto: per toglierlo usa «Rimuovi»".to_string());
    }
    if value.len() > SECRET_MAX {
        return Err(format!("il valore supera i {SECRET_MAX} caratteri"));
    }
    if !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("il valore contiene spazi o caratteri non validi".to_string());
    }
    Ok(value)
}

impl InstanceSecrets {
    pub fn new(store: Arc<dyn SecretStore>, instance: &str) -> Result<Self, String> {
        if !valid_instance_id(instance) {
            return Err("id istanza non valido".to_string());
        }
        Ok(Self { store, instance: instance.to_string() })
    }

    fn account(&self, slot: &str) -> Result<String, String> {
        account(&self.instance, slot).ok_or_else(|| "slot dei segreti non valido".to_string())
    }

    /// Il segreto, `None` se non c'e'. Solo per il backend (il connettore).
    pub fn get(&self, slot: &str) -> Result<Option<String>, String> {
        self.store.get(&self.account(slot)?)
    }

    /// Valida e salva il segreto (sostituisce il precedente).
    pub fn set(&self, slot: &str, raw: &str) -> Result<(), String> {
        let account = self.account(slot)?;
        self.store.set(&account, validate_value(raw)?)
    }

    /// Toglie il segreto; nessun errore se non c'era.
    pub fn clear(&self, slot: &str) -> Result<(), String> {
        self.store.delete(&self.account(slot)?)
    }

    pub fn is_set(&self, slot: &str) -> Result<bool, String> {
        Ok(self.get(slot)?.is_some())
    }

    /// Per ogni slot, se e' impostato: cio' che vede il frontend.
    #[cfg(test)]
    pub fn view(&self, slots: &[&str]) -> Result<Vec<SecretSlot>, String> {
        slots.iter().map(|slot| Ok(SecretSlot { slot: (*slot).to_string(), set: self.is_set(slot)? })).collect()
    }

    /// Toglie tutti gli slot dichiarati. Prova ogni slot anche se uno fallisce; se
    /// qualcuno fallisce, l'errore elenca gli slot (mai i valori).
    pub fn delete_all(&self, slots: &[&str]) -> Result<(), String> {
        let failed: Vec<&str> = slots.iter().copied().filter(|slot| self.clear(slot).is_err()).collect();
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!("segreti non rimossi dal portachiavi: {}", failed.join(", ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecrets;

    const INSTANCE: &str = "con-1700000000000-1";

    fn secrets() -> (Arc<MemorySecrets>, InstanceSecrets) {
        let store = Arc::new(MemorySecrets::default());
        let secrets = InstanceSecrets::new(store.clone(), INSTANCE).unwrap();
        (store, secrets)
    }

    #[test]
    fn accounts_are_namespaced_by_instance_and_slot() {
        assert_eq!(account(INSTANCE, "api_token").as_deref(), Some("connector-con-1700000000000-1-api_token"));
        assert_eq!(account("con-bad", "api_token"), None);
        assert_eq!(account(INSTANCE, "Bad Slot"), None);
        assert_eq!(account(INSTANCE, ""), None);
        assert!(InstanceSecrets::new(Arc::new(MemorySecrets::default()), "../x").is_err());
    }

    #[test]
    fn set_get_clear_roundtrip_with_trim() {
        let (store, secrets) = secrets();
        assert_eq!(secrets.get("api_token").unwrap(), None);
        secrets.set("api_token", "  tok-123 \n").unwrap();
        assert_eq!(secrets.get("api_token").unwrap().as_deref(), Some("tok-123"));
        assert_eq!(store.get("connector-con-1700000000000-1-api_token").unwrap().as_deref(), Some("tok-123"));
        secrets.set("api_token", "tok-456").unwrap();
        assert_eq!(secrets.get("api_token").unwrap().as_deref(), Some("tok-456"));
        secrets.clear("api_token").unwrap();
        secrets.clear("api_token").unwrap();
        assert!(!secrets.is_set("api_token").unwrap());
    }

    #[test]
    fn values_are_validated_and_errors_never_echo_them() {
        let (_, secrets) = secrets();
        for bad in ["", "   ", "con spazio", "con\ttab", "è", &"x".repeat(SECRET_MAX + 1)] {
            let err = secrets.set("api_token", bad).unwrap_err();
            assert!(!err.contains(bad) || bad.is_empty() || bad.trim().is_empty(), "{err}");
            assert!(!secrets.is_set("api_token").unwrap(), "{bad:?}");
        }
        assert!(secrets.set("api_token", &"x".repeat(SECRET_MAX)).is_ok());
        let err = secrets.set("api_token", "tok-SEGRETO con spazio").unwrap_err();
        assert!(!err.contains("SEGRETO"), "{err}");
        assert!(secrets.set("bad slot", "tok").is_err());
    }

    #[test]
    fn the_view_has_only_slot_and_set_and_never_the_value() {
        let (_, secrets) = secrets();
        secrets.set("api_token", "tok-SEGRETO").unwrap();
        let view = secrets.view(&["api_token", "refresh_token"]).unwrap();
        assert_eq!(
            view,
            vec![
                SecretSlot { slot: "api_token".into(), set: true },
                SecretSlot { slot: "refresh_token".into(), set: false }
            ]
        );
        assert!(!serde_json::to_string(&view).unwrap().contains("SEGRETO"));
        assert!(!format!("{view:?}").contains("SEGRETO"));
    }

    #[test]
    fn instances_do_not_see_each_other_and_delete_all_clears_every_slot() {
        let store = Arc::new(MemorySecrets::default());
        let a = InstanceSecrets::new(store.clone(), "con-1-1").unwrap();
        let b = InstanceSecrets::new(store, "con-1-2").unwrap();
        a.set("api_token", "tok-a").unwrap();
        a.set("refresh_token", "ref-a").unwrap();
        b.set("api_token", "tok-b").unwrap();
        a.delete_all(&["api_token", "refresh_token", "never_set"]).unwrap();
        assert!(!a.is_set("api_token").unwrap() && !a.is_set("refresh_token").unwrap());
        assert_eq!(b.get("api_token").unwrap().as_deref(), Some("tok-b"));
    }

    /// Un portachiavi che rifiuta una cancellazione: le altre si fanno comunque.
    struct FailingDelete(MemorySecrets, &'static str);

    impl SecretStore for FailingDelete {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            self.0.get(account)
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            self.0.set(account, secret)
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            if account.ends_with(self.1) {
                return Err("accesso negato".to_string());
            }
            self.0.delete(account)
        }
    }

    #[test]
    fn delete_all_tries_every_slot_and_reports_only_the_failed_names() {
        let store = Arc::new(FailingDelete(MemorySecrets::default(), "-api_token"));
        let secrets = InstanceSecrets::new(store, INSTANCE).unwrap();
        secrets.set("api_token", "tok-SEGRETO").unwrap();
        secrets.set("refresh_token", "ref-SEGRETO").unwrap();
        let err = secrets.delete_all(&["api_token", "refresh_token"]).unwrap_err();
        assert_eq!(err, "segreti non rimossi dal portachiavi: api_token");
        assert!(!err.contains("SEGRETO"));
        assert!(!secrets.is_set("refresh_token").unwrap(), "l'altro slot e' stato tolto comunque");
    }
}
