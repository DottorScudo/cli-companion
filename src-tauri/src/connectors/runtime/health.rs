//! La salute di ogni istanza: lo stato che l'interfaccia mostra
//! (`connector-status`, l'elenco intero a ogni cambiamento). Sta in memoria, non
//! su disco. `last_error` e' solo un testo generico in italiano scelto dalla
//! classe dell'errore (`ConnectorError::public_message`) o una costante: il tipo
//! `&'static str` impedisce a un testo del servizio, un URL o un segreto di
//! arrivarci. Il dettaglio va nel log, mai qui.

use serde::Serialize;
use std::collections::HashMap;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

/// Evento Tauri con l'elenco completo degli stati.
pub const STATUS_EVENT: &str = "connector-status";

/// In che stato e' un'istanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "ConnectorHealthState")]
pub enum HealthState {
    /// Spenta (dall'utente o perche' l'HUB e' spento).
    Disabled,
    /// In partenza: primo giro non ancora riuscito.
    Starting,
    Ok,
    /// Un errore transitorio: nuovo tentativo a `next_retry_at`.
    Backoff,
    /// Credenziali rifiutate: serve l'intervento dell'utente.
    NeedsAuth,
    /// Configurazione non valida o errore non recuperabile: si e' fermata.
    Error,
    /// Il connettore dell'istanza non e' compilato in questa versione dell'app.
    Unavailable,
}

/// Lo stato di un'istanza, come lo vede l'interfaccia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorStatus")]
pub struct ConnectorStatus {
    pub instance: String,
    pub state: HealthState,
    /// L'ultimo giro riuscito (millisecondi Unix).
    pub last_ok_at: Option<i64>,
    /// Testo generico in italiano: mai risposte del servizio, URL o segreti.
    pub last_error: Option<String>,
    /// Quando si riprova, se si sta aspettando (millisecondi Unix).
    pub next_retry_at: Option<i64>,
}

/// Chi riceve l'elenco a ogni cambiamento: in produzione l'evento Tauri, nei test un registratore.
pub trait StatusEmitter: Send + Sync {
    fn emit(&self, statuses: &[ConnectorStatus]);
}

struct NoEmitter;

impl StatusEmitter for NoEmitter {
    fn emit(&self, _: &[ConnectorStatus]) {}
}

/// Emette `connector-status` verso il frontend.
pub struct TauriStatusEmitter(pub AppHandle);

impl StatusEmitter for TauriStatusEmitter {
    fn emit(&self, statuses: &[ConnectorStatus]) {
        if let Err(e) = self.0.emit(STATUS_EVENT, statuses) {
            log::error!("connettori: evento {STATUS_EVENT} non inviato: {e}");
        }
    }
}

/// Gli stati di tutte le istanze. Ogni esecuzione di un'istanza ha un'epoca
/// (`begin_run`): solo la corrente puo' scrivere lo stato con i metodi `run_*`;
/// `end_run` (l'esecuzione e' stata fermata) la invalida, cosi' un task interrotto
/// ma ancora in volo non sovrascrive mai lo stato deciso dal manager.
pub struct StatusRegistry {
    map: Mutex<BTreeMap<String, ConnectorStatus>>,
    emitter: Mutex<Arc<dyn StatusEmitter>>,
    epochs: Mutex<HashMap<String, u64>>,
    next_epoch: AtomicU64,
}

impl Default for StatusRegistry {
    fn default() -> Self {
        Self {
            map: Mutex::new(BTreeMap::new()),
            emitter: Mutex::new(Arc::new(NoEmitter)),
            epochs: Mutex::new(HashMap::new()),
            next_epoch: AtomicU64::new(0),
        }
    }
}

impl StatusRegistry {
    pub fn set_emitter(&self, emitter: Arc<dyn StatusEmitter>) {
        *self.emitter.lock().unwrap_or_else(PoisonError::into_inner) = emitter;
    }

    /// L'elenco, in ordine di id.
    pub fn list(&self) -> Vec<ConnectorStatus> {
        self.lock().values().cloned().collect()
    }

    pub fn get(&self, instance: &str) -> Option<ConnectorStatus> {
        self.lock().get(instance).cloned()
    }

    /// Uno stato senza errore (spenta, in partenza, non disponibile). L'ultimo successo si conserva.
    pub fn set_state(&self, instance: &str, state: HealthState) {
        self.change(instance, |s| {
            s.state = state;
            s.last_error = None;
            s.next_retry_at = None;
        });
    }

    /// Un giro riuscito.
    #[cfg(test)]
    pub fn mark_ok(&self, instance: &str, now_ms: i64) {
        self.change(instance, |s| {
            s.state = HealthState::Ok;
            s.last_ok_at = Some(now_ms);
            s.last_error = None;
            s.next_retry_at = None;
        });
    }

    /// Un errore: `message` e' sempre un testo generico fisso.
    pub fn mark_failed(&self, instance: &str, state: HealthState, message: &'static str, next_retry_at: Option<i64>) {
        self.change(instance, |s| {
            s.state = state;
            s.last_error = Some(message.to_string());
            s.next_retry_at = next_retry_at;
        });
    }

    /// Inizia una nuova esecuzione di `instance`: le precedenti non scrivono piu'.
    pub fn begin_run(&self, instance: &str) -> u64 {
        let epoch = self.next_epoch.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        self.epochs().insert(instance.to_string(), epoch);
        epoch
    }

    /// L'esecuzione corrente di `instance` e' finita o e' stata fermata: nessuna epoca scrive piu'.
    pub fn end_run(&self, instance: &str) {
        self.epochs().remove(instance);
    }

    /// Come `set_state`, ma solo se `epoch` e' l'esecuzione corrente.
    pub fn run_state(&self, instance: &str, epoch: u64, state: HealthState) {
        self.run_change(instance, epoch, |s| {
            s.state = state;
            s.last_error = None;
            s.next_retry_at = None;
        });
    }

    /// Come `mark_ok`, ma solo se `epoch` e' l'esecuzione corrente.
    pub fn run_ok(&self, instance: &str, epoch: u64, now_ms: i64) {
        self.run_change(instance, epoch, |s| {
            s.state = HealthState::Ok;
            s.last_ok_at = Some(now_ms);
            s.last_error = None;
            s.next_retry_at = None;
        });
    }

    /// Come `mark_failed`, ma solo se `epoch` e' l'esecuzione corrente.
    pub fn run_failed(
        &self,
        instance: &str,
        epoch: u64,
        state: HealthState,
        message: &'static str,
        next_retry_at: Option<i64>,
    ) {
        self.run_change(instance, epoch, |s| {
            s.state = state;
            s.last_error = Some(message.to_string());
            s.next_retry_at = next_retry_at;
        });
    }

    fn run_change(&self, instance: &str, epoch: u64, edit: impl FnOnce(&mut ConnectorStatus)) {
        // Il lock delle epoche resta preso durante la scrittura: `end_run` non puo' passare nel mezzo.
        let epochs = self.epochs();
        if epochs.get(instance) == Some(&epoch) {
            self.change(instance, edit);
        }
    }

    fn epochs(&self) -> std::sync::MutexGuard<'_, HashMap<String, u64>> {
        self.epochs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn remove(&self, instance: &str) {
        self.end_run(instance);
        let removed = self.lock().remove(instance).is_some();
        if removed {
            self.emit();
        }
    }

    /// Toglie gli stati delle istanze che non ci sono piu'.
    pub fn retain(&self, live: &HashSet<String>) {
        let before = self.lock().len();
        self.lock().retain(|id, _| live.contains(id));
        if self.lock().len() != before {
            self.emit();
        }
    }

    fn change(&self, instance: &str, edit: impl FnOnce(&mut ConnectorStatus)) {
        let changed = {
            let mut map = self.lock();
            let created = !map.contains_key(instance);
            let entry = map.entry(instance.to_string()).or_insert_with(|| ConnectorStatus {
                instance: instance.to_string(),
                state: HealthState::Disabled,
                last_ok_at: None,
                last_error: None,
                next_retry_at: None,
            });
            let before = entry.clone();
            edit(entry);
            created || *entry != before
        };
        if changed {
            self.emit();
        }
    }

    /// Emette l'elenco intero, fuori dal lock.
    fn emit(&self) {
        let emitter = self.emitter.lock().unwrap_or_else(PoisonError::into_inner).clone();
        emitter.emit(&self.list());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, ConnectorStatus>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
