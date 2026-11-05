//! I DTO dei comandi verso il frontend. Nessun segreto passa da qui: dei segreti
//! si vede solo se uno slot e' impostato.

use crate::connectors::model::ConnectorInstance;
use crate::connectors::registry;
use crate::connectors::runtime::health::{ConnectorStatus, HealthState};
use crate::connectors::runtime::manager::ConnectorsRuntime;
use crate::connectors::secrets::SecretSlot;
use serde::Serialize;
use ts_rs::TS;

/// Un'istanza con cio' che l'interfaccia mostra: gli slot dei segreti (impostato
/// si'/no, mai il valore) e lo stato di salute.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, rename = "ConnectorInstanceView")]
pub struct InstanceView {
    pub instance: ConnectorInstance,
    /// Tutti gli slot dichiarati dal descrittore (campi `secret` e slot interni); vuoto se il connettore non e' compilato.
    pub secrets: Vec<SecretSlot>,
    pub status: ConnectorStatus,
}

/// L'esito di un'azione dell'interfaccia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorActionOutcome")]
pub struct ActionResult {
    /// Il messaggio per l'utente, in italiano.
    pub message: String,
    /// Un dato da mostrare (un link di abbinamento, il nome del bot), se c'e'.
    pub detail: Option<String>,
    /// Il codice monouso emesso dall'azione `pair` (vale 10 minuti): l'utente lo manda dall'account da abbinare.
    pub pairing_code: Option<String>,
}

/// L'esito dell'eliminazione: l'istanza e' sempre rimossa; `warning` dice cosa non si e' potuto pulire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorDeleteOutcome")]
pub struct DeleteResult {
    pub warning: Option<String>,
}

/// Lo stato di salute di un'istanza che il registro non conosce ancora (spenta).
pub fn default_status(instance: &str) -> ConnectorStatus {
    ConnectorStatus {
        instance: instance.to_string(),
        state: HealthState::Disabled,
        last_ok_at: None,
        last_error: None,
        next_retry_at: None,
    }
}

pub fn assemble(
    instance: ConnectorInstance,
    secrets: Vec<SecretSlot>,
    status: Option<ConnectorStatus>,
) -> InstanceView {
    let status = status.unwrap_or_else(|| default_status(&instance.id));
    InstanceView { instance, secrets, status }
}

/// La vista di un'istanza. Legge il portachiavi: bloccante. Uno slot illeggibile
/// risulta non impostato (e si scrive nel log), cosi' un portachiavi guasto non
/// nasconde le altre istanze.
pub fn build_view(rt: &ConnectorsRuntime, instance: ConnectorInstance) -> InstanceView {
    let slots = registry::find(&instance.connector).map(|c| c.descriptor().secret_slots()).unwrap_or_default();
    let secrets = match rt.instance_secrets(&instance.id) {
        Ok(secrets) => slots
            .iter()
            .map(|slot| SecretSlot {
                slot: (*slot).to_string(),
                set: secrets.is_set(slot).unwrap_or_else(|e| {
                    log::warn!("connettori {}: portachiavi non leggibile: {e}", instance.id);
                    false
                }),
            })
            .collect(),
        Err(e) => {
            log::warn!("connettori {}: {e}", instance.id);
            Vec::new()
        }
    };
    let status = rt.status_registry().get(&instance.id);
    assemble(instance, secrets, status)
}
