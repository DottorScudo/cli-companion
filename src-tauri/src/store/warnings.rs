//! Avvisi sui file di dati: file messo da parte, non leggibile, di una
//! versione piu' recente. Restano in memoria (ultimi `MAX_WARNINGS`) e, se
//! installato l'emettitore, vengono notificati al frontend.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

/// Avvisi conservati in memoria: i piu' vecchi vengono scartati.
const MAX_WARNINGS: usize = 100;
/// Evento Tauri emesso per ogni nuovo avviso.
const WARNING_EVENT: &str = "store-warning";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// File danneggiato o non migrabile: messo da parte, si riparte dal default.
    Corrupt,
    /// File scritto da una versione piu' recente dell'app: messo da parte.
    NewerVersion,
    /// File non leggibile (permessi, bloccato): non toccato, salvataggi rifiutati.
    Unreadable,
    /// Alcune voci non erano utilizzabili e sono state escluse; le altre sono
    /// caricate e il file originale e' copiato in un backup.
    PartiallyRead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreWarning {
    pub file: String,
    pub kind: WarningKind,
    /// Testo per l'utente.
    pub message: String,
    /// Nome del backup, se il file e' stato messo da parte.
    pub backup: Option<String>,
    /// Unix ms.
    pub at: i64,
}

type WarningEmitter = Box<dyn Fn(&StoreWarning) + Send + Sync>;

pub(crate) struct WarningLog {
    items: Mutex<Vec<StoreWarning>>,
    emitter: OnceLock<WarningEmitter>,
}

impl WarningLog {
    pub(crate) const fn new() -> Self {
        Self { items: Mutex::new(Vec::new()), emitter: OnceLock::new() }
    }

    /// Registra l'avviso (scartando i piu' vecchi oltre il tetto) e lo passa
    /// all'emettitore, fuori dal lock.
    pub(crate) fn record(&self, warning: StoreWarning) {
        {
            let mut items = self.items.lock().unwrap();
            items.push(warning.clone());
            let excess = items.len().saturating_sub(MAX_WARNINGS);
            items.drain(..excess);
        }
        if let Some(emit) = self.emitter.get() {
            emit(&warning);
        }
    }

    pub(crate) fn snapshot(&self) -> Vec<StoreWarning> {
        self.items.lock().unwrap().clone()
    }

    /// Installa l'emettitore (una sola volta; le chiamate successive sono ignorate).
    pub(crate) fn set_emitter(&self, emit: impl Fn(&StoreWarning) + Send + Sync + 'static) {
        let _ = self.emitter.set(Box::new(emit));
    }
}

static WARNINGS: WarningLog = WarningLog::new();

/// Registra un avviso nel log di processo.
pub(crate) fn report(warning: StoreWarning) {
    WARNINGS.record(warning);
}

/// Da qui in poi ogni avviso viene emesso anche come evento `store-warning`.
pub fn install_emitter(app: &AppHandle) {
    let app = app.clone();
    WARNINGS.set_emitter(move |w| {
        if let Err(e) = app.emit(WARNING_EVENT, w) {
            log::warn!("impossibile notificare l'avviso su {}: {e}", w.file);
        }
    });
}

/// Avvisi registrati finora (per il frontend, che si e' avviato dopo il caricamento).
#[tauri::command]
pub fn store_warnings() -> Vec<StoreWarning> {
    WARNINGS.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn warning(n: i64) -> StoreWarning {
        StoreWarning {
            file: format!("f{n}.json"),
            kind: WarningKind::Corrupt,
            message: format!("avviso {n}"),
            backup: None,
            at: n,
        }
    }

    #[test]
    fn keeps_order_and_caps_dropping_the_oldest() {
        let log = WarningLog::new();
        let total = MAX_WARNINGS as i64 + 5;
        for n in 0..total {
            log.record(warning(n));
        }
        let snap = log.snapshot();
        assert_eq!(snap.len(), MAX_WARNINGS);
        assert_eq!(snap.first().unwrap().at, 5);
        assert_eq!(snap.last().unwrap().at, total - 1);
    }

    #[test]
    fn emitter_receives_every_warning_once_installed() {
        let log = WarningLog::new();
        log.record(warning(0));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        log.set_emitter(move |w| sink.lock().unwrap().push(w.at));
        log.record(warning(1));
        log.record(warning(2));
        assert_eq!(*seen.lock().unwrap(), vec![1, 2]);
        assert_eq!(log.snapshot().len(), 3);
    }

    #[test]
    fn serializes_for_the_frontend() {
        let w = StoreWarning { backup: Some("a.json.bak-1".into()), kind: WarningKind::NewerVersion, ..warning(1) };
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["kind"], "newer_version");
        assert_eq!(v["backup"], "a.json.bak-1");
        assert_eq!(v["file"], "f1.json");
        assert_eq!(serde_json::to_value(WarningKind::Unreadable).unwrap(), "unreadable");
        assert_eq!(serde_json::to_value(WarningKind::Corrupt).unwrap(), "corrupt");
        assert_eq!(serde_json::to_value(WarningKind::PartiallyRead).unwrap(), "partially_read");
    }
}
