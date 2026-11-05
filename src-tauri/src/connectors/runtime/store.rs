//! Le due memorie persistenti del runtime, dietro tipi semplici da sostituire nei test:
//! - `ConnectorsStore`: `connectors.json` in memoria con un lock; ogni modifica si
//!   salva su disco prima di essere pubblicata (come `store::JsonStore`), senza
//!   cartella (test) resta in memoria. Le modifiche I/O vanno chiamate dal pool bloccante.
//! - `StateStore`: lo stato di un'istanza (`connector_state/<istanza>.json`).

use crate::connectors::contracts::BoxFuture;
use crate::connectors::files::{self, StateFile};
use crate::connectors::model::{ConnectorInstance, ConnectorsFile};
use crate::connectors::state::InstanceState;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

#[derive(Default)]
pub struct ConnectorsStore {
    data: Mutex<ConnectorsFile>,
    /// Serializza le modifiche: l'I/O avviene fuori dal lock `data`.
    save: Mutex<()>,
    dir: OnceLock<PathBuf>,
}

impl ConnectorsStore {
    /// La cartella dati dove salvare; senza, le modifiche restano in memoria.
    pub fn attach_dir(&self, dir: &Path) {
        if self.dir.set(dir.to_path_buf()).is_err() {
            log::warn!("connettori: cartella dati gia' impostata");
        }
    }

    /// Carica `connectors.json` (avvisi e quarantena li gestisce `files::load`).
    pub fn load_from_disk(&self, dir: &Path) {
        self.attach_dir(dir);
        let loaded = files::load(dir).value;
        *self.lock() = loaded;
    }

    /// Sostituisce il contenuto in memoria (caricamento, test).
    pub fn replace(&self, file: ConnectorsFile) {
        *self.lock() = file;
    }

    pub fn snapshot(&self) -> ConnectorsFile {
        self.lock().clone()
    }

    pub fn read<R>(&self, f: impl FnOnce(&ConnectorsFile) -> R) -> R {
        f(&self.lock())
    }

    pub fn instance(&self, id: &str) -> Option<ConnectorInstance> {
        self.read(|f| f.instance(id).cloned())
    }

    /// Applica `f` a una copia, la salva e solo poi la pubblica: se `f` o il
    /// salvataggio falliscono nulla cambia.
    pub fn update(&self, f: impl FnOnce(&mut ConnectorsFile) -> Result<(), String>) -> Result<ConnectorsFile, String> {
        let _save = self.save.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = self.snapshot();
        f(&mut next)?;
        if let Some(dir) = self.dir.get() {
            files::save(dir, &next)?;
        }
        *self.lock() = next.clone();
        Ok(next)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ConnectorsFile> {
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Dove vive lo stato delle istanze.
pub trait StateStore: Send + Sync {
    /// Lo stato salvato; assente o illeggibile = default (il file corrotto e' gia' in quarantena).
    fn load<'a>(&'a self, instance: &'a str) -> BoxFuture<'a, InstanceState>;
    fn save<'a>(&'a self, instance: &'a str, state: &'a InstanceState) -> BoxFuture<'a, Result<(), String>>;
}

/// Lo stato su disco, nella cartella dati dell'app.
pub struct FileStateStore {
    pub data_dir: PathBuf,
}

impl StateStore for FileStateStore {
    fn load<'a>(&'a self, instance: &'a str) -> BoxFuture<'a, InstanceState> {
        let (dir, instance) = (self.data_dir.clone(), instance.to_string());
        Box::pin(async move {
            let loaded = crate::blocking(move || Ok(StateFile::open(&dir, &instance)?.load().value)).await;
            loaded.unwrap_or_else(|e| {
                log::warn!("connettori: stato non letto, si riparte da zero: {e}");
                InstanceState::default()
            })
        })
    }

    fn save<'a>(&'a self, instance: &'a str, state: &'a InstanceState) -> BoxFuture<'a, Result<(), String>> {
        let (dir, instance, state) = (self.data_dir.clone(), instance.to_string(), state.clone());
        Box::pin(crate::blocking(move || StateFile::open(&dir, &instance)?.save(&state)))
    }
}

/// `ConnectorsStore::update` nel pool bloccante, per il codice asincrono dei supervisori.
pub async fn update_blocking(
    store: &std::sync::Arc<ConnectorsStore>,
    f: impl FnOnce(&mut ConnectorsFile) -> Result<(), String> + Send + 'static,
) -> Result<ConnectorsFile, String> {
    let store = store.clone();
    crate::blocking(move || store.update(f)).await
}
