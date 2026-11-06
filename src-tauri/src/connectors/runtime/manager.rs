//! `ConnectorsRuntime`: lo stato gestito da Tauri che possiede `connectors.json`
//! in memoria, gli stati di salute e un task per ogni istanza abilitata. Decide
//! chi gira: solo con l'HUB acceso, solo le istanze abilitate il cui connettore e'
//! nel registro e la cui configurazione (migrata) e' valida. Tutto cio' che tocca
//! `AppHandle` sta in `load`; il resto lavora sull'`Env` iniettato, cosi' i test
//! girano con finti.
//!
//! Chi lo chiama (il ciclo di vita dell'HUB, i comandi): `set_hub_enabled`,
//! `reconcile` dopo una modifica del file (`mutate` lo fa da se'),
//! `restart_instance` dopo aver cambiato configurazione o segreti,
//! `stop_all` all'uscita. Un task fermato da un errore (`needs_auth`, `error`)
//! resta registrato e NON riparte da solo: serve `restart_instance`.

use crate::connectors::contracts::{ConnectorContext, ConnectorImpl, SystemClock};
use crate::connectors::model::ConnectorsFile;
use crate::connectors::registry;
use crate::connectors::runtime::contact::subchats::ThreadKey;
use crate::connectors::runtime::health::{ConnectorStatus, HealthState, StatusRegistry, TauriStatusEmitter};
use crate::connectors::runtime::hub_port::TauriHubPort;
use crate::connectors::runtime::migrate::{migrate_instance, validate_instance};
use crate::connectors::runtime::notices::NoticeRouter;
use crate::connectors::runtime::pairing::PairingRegistry;
use crate::connectors::runtime::store::{ConnectorsStore, FileStateStore, StateStore};
use crate::connectors::runtime::supervisor::{build_context, run, Env, Job};
use crate::connectors::runtime::time::{Sleeper, TokioSleeper};
use crate::connectors::secrets::InstanceSecrets;
use crate::connectors::state::InstanceState;
use crate::connectors::{contracts::StateHandle, ids::IdGenerator};
use crate::hub::Channel;
use crate::secrets::{OsKeyring, SecretStore};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use tauri::async_runtime::JoinHandle;
use tauri::AppHandle;

/// I testi fissi dello stato `error` all'avvio.
const MIGRATION_FAILED: &str = "Configurazione di una versione precedente non aggiornabile.";
const CONFIG_INVALID: &str = "Configurazione non valida.";

/// Le dipendenze esterne del runtime (diverse in produzione e nei test).
pub struct Platform {
    pub hub: Arc<dyn crate::connectors::contracts::HubPort>,
    pub clock: Arc<dyn crate::connectors::contracts::Clock>,
    pub sleeper: Arc<dyn Sleeper>,
    pub states: Arc<dyn StateStore>,
    pub secrets: Arc<dyn SecretStore>,
}

struct Running {
    handle: JoinHandle<()>,
    state: StateHandle,
}

pub struct ConnectorsRuntime {
    store: Arc<ConnectorsStore>,
    statuses: Arc<StatusRegistry>,
    notices: Arc<NoticeRouter>,
    pairing: Arc<PairingRegistry>,
    env: OnceLock<Arc<Env>>,
    hub_enabled: AtomicBool,
    running: Mutex<HashMap<String, Running>>,
    /// Istanze la cui migrazione e' fallita: restano com'erano e vanno in `error`.
    migration_failed: Mutex<HashSet<String>>,
}

impl Default for ConnectorsRuntime {
    fn default() -> Self {
        let store = Arc::new(ConnectorsStore::default());
        Self {
            notices: Arc::new(NoticeRouter::new(store.clone())),
            store,
            statuses: Arc::new(StatusRegistry::default()),
            pairing: Arc::new(PairingRegistry::default()),
            env: OnceLock::new(),
            hub_enabled: AtomicBool::new(false),
            running: Mutex::new(HashMap::new()),
            migration_failed: Mutex::new(HashSet::new()),
        }
    }
}

impl ConnectorsRuntime {
    /// Carica `connectors.json`, collega la porta HUB vera e prepara le istanze
    /// (migrazione in memoria). Non avvia nulla: lo fa `set_hub_enabled`/`reconcile`.
    /// Bloccante: da `setup` o dal pool bloccante.
    pub fn load(&self, app: &AppHandle) {
        let dir = match crate::store::data_dir(app) {
            Ok(dir) => dir,
            Err(e) => {
                log::error!("connettori: cartella dati non disponibile, runtime spento: {e}");
                return;
            }
        };
        self.store.load_from_disk(&dir);
        self.statuses.set_emitter(Arc::new(TauriStatusEmitter(app.clone())));
        self.install(Platform {
            hub: Arc::new(TauriHubPort(app.clone())),
            clock: Arc::new(SystemClock),
            sleeper: Arc::new(TokioSleeper),
            states: Arc::new(FileStateStore { data_dir: dir }),
            secrets: Arc::new(OsKeyring),
        });
    }

    /// Imposta le dipendenze (una volta) e migra le configurazioni in memoria.
    pub fn install(&self, platform: Platform) {
        let env = Env {
            hub: platform.hub,
            clock: platform.clock,
            sleeper: platform.sleeper,
            store: self.store.clone(),
            states: platform.states,
            statuses: self.statuses.clone(),
            secrets: platform.secrets,
            client: OnceLock::new(),
            pairing: self.pairing.clone(),
            ids: IdGenerator::default(),
        };
        if self.env.set(Arc::new(env)).is_err() {
            log::warn!("connettori: runtime gia' installato");
            return;
        }
        self.migrate_all();
    }

    /// Il destinatario delle notifiche dell'HUB (da registrare con `hub::set_notice_sink`).
    pub fn notice_sink(&self) -> Arc<NoticeRouter> {
        self.notices.clone()
    }

    pub fn store(&self) -> &Arc<ConnectorsStore> {
        &self.store
    }

    pub fn snapshot(&self) -> ConnectorsFile {
        self.store.snapshot()
    }

    pub fn statuses(&self) -> Vec<ConnectorStatus> {
        self.statuses.list()
    }

    pub fn status_registry(&self) -> &Arc<StatusRegistry> {
        &self.statuses
    }

    pub fn hub_enabled(&self) -> bool {
        self.hub_enabled.load(Ordering::SeqCst)
    }

    /// Gli id delle istanze con un task registrato (anche uno fermato da un errore).
    pub fn running_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.running().keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Un id nuovo (`con-...`, `rte-...`, `lnk-...`), unico anche nello stesso millisecondo.
    pub fn new_id(&self, prefix: &str) -> Result<String, String> {
        let env = self.env()?;
        Ok(env.ids.next(prefix, env.clock.now_ms()))
    }

    /// L'ora del runtime (millisecondi Unix).
    pub fn now_ms(&self) -> Result<i64, String> {
        Ok(self.env()?.clock.now_ms())
    }

    /// I segreti di un'istanza nel portachiavi (bloccante: va usato dal pool bloccante).
    pub fn instance_secrets(&self, instance: &str) -> Result<InstanceSecrets, String> {
        InstanceSecrets::new(self.env()?.secrets.clone(), instance)
    }

    /// Un codice di abbinamento nuovo per un canale di contatto (azione `pair`).
    pub fn issue_pairing_code(&self, instance: &str) -> Result<String, String> {
        let env = self.env()?;
        self.pairing.issue(instance, &*env.clock)
    }

    /// L'HUB e' stato acceso o spento: accesa avvia le istanze, spenta ferma tutto.
    pub fn set_hub_enabled(&self, enabled: bool) {
        self.hub_enabled.store(enabled, Ordering::SeqCst);
        if enabled {
            self.reconcile();
        } else {
            self.stop_all();
        }
    }

    /// Allinea i task al file: avvia le istanze abilitate e disponibili che non girano,
    /// ferma quelle spente o scomparse, aggiorna gli stati. Rapido, non fa I/O.
    pub fn reconcile(&self) {
        let Some(env) = self.env.get().cloned() else { return };
        let file = self.store.snapshot();
        let live: HashSet<String> = file.instances.iter().map(|i| i.id.clone()).collect();
        self.statuses.retain(&live);
        let hub_on = self.hub_enabled();
        let stale: Vec<String> = self
            .running()
            .keys()
            .filter(|id| {
                !hub_on || file.instance(id).is_none_or(|i| !i.enabled || registry::find(&i.connector).is_none())
            })
            .cloned()
            .collect();
        for id in stale {
            self.stop_task(&id);
        }
        for instance in &file.instances {
            if !hub_on {
                self.statuses.set_state(&instance.id, HealthState::Disabled);
            } else if registry::find(&instance.connector).is_none() {
                self.statuses.set_state(&instance.id, HealthState::Unavailable);
            } else if !instance.enabled {
                self.statuses.set_state(&instance.id, HealthState::Disabled);
            } else if !self.running().contains_key(&instance.id) {
                self.start(&env, &instance.id);
            }
        }
    }

    /// Ferma e riavvia un'istanza (dopo una modifica della configurazione o dei segreti).
    pub fn restart_instance(&self, id: &str) {
        self.stop_task(id);
        self.migrate_one(id);
        self.reconcile();
    }

    /// Ferma tutto (HUB spento, uscita dall'app): i task si interrompono, le
    /// notifiche in coda si perdono, tutti gli stati sono `disabled`.
    pub fn stop_all(&self) {
        let ids: Vec<String> = self.running().keys().cloned().collect();
        for id in ids {
            self.stop_task(&id);
        }
        self.notices.clear();
        for instance in self.store.snapshot().instances {
            self.statuses.set_state(&instance.id, HealthState::Disabled);
        }
    }

    /// Modifica `connectors.json` (salvato su disco: va chiamata dal pool bloccante)
    /// e riallinea i task.
    pub fn mutate(&self, f: impl FnOnce(&mut ConnectorsFile) -> Result<(), String>) -> Result<ConnectorsFile, String> {
        let file = self.store.update(f)?;
        // Le conversazioni e i cursori delle regole tolte non servono piu'.
        let live: HashSet<String> = file.routes.iter().map(|r| r.id.clone()).collect();
        for running in self.running().values() {
            running.state.lock().prune_routes(&live);
        }
        self.reconcile();
        Ok(file)
    }

    /// Il contesto di un'istanza per un'azione dell'interfaccia o una prova: condivide
    /// lo stato col task in esecuzione, altrimenti lo carica dal disco.
    pub async fn context_for(&self, instance_id: &str) -> Result<(ConnectorContext, ConnectorImpl), String> {
        let env = self.env()?;
        let instance = self.store.instance(instance_id).ok_or("istanza inesistente")?;
        let imp = registry::find(&instance.connector).ok_or("connettore non disponibile in questa versione")?;
        let running_state = self.running().get(instance_id).map(|r| r.state.clone());
        let state = match running_state {
            Some(state) => state,
            None => StateHandle::new(env.states.load(instance_id).await),
        };
        Ok((build_context(&env, &instance, imp.descriptor(), state)?, imp))
    }

    /// Un canale non esiste piu' (workspace rimosso): via le sue regole e il thread
    /// corrente dei collegamenti su quel canale; le conversazioni delle regole
    /// tolte si potano dallo stato dei task in esecuzione (quello degli altri, al
    /// prossimo avvio). Non crea il file se non c'e' nulla da togliere. Bloccante.
    pub fn forget_channel(&self, channel: &Channel) -> Result<(), String> {
        let affected = self.store.read(|f| {
            f.routes.iter().any(|r| r.channel == *channel)
                || f.links.iter().any(|l| l.current.as_ref().is_some_and(|c| c.channel == *channel))
        });
        if !affected {
            return Ok(());
        }
        let file = self.store.update(|f| {
            let forgotten = f.forget_channel(channel);
            log::info!(
                "connettori: canale rimosso, {} regole tolte, {} collegamenti azzerati",
                forgotten.routes.len(),
                forgotten.links_reset
            );
            Ok(())
        })?;
        let live: HashSet<String> = file.routes.iter().map(|r| r.id.clone()).collect();
        for running in self.running().values() {
            running.state.lock().prune_routes(&live);
        }
        Ok(())
    }

    /// Un thread e' stato eliminato: le istanze di contatto in esecuzione ne eliminano le
    /// sotto-chat. Rapido, non fa I/O: il lavoro di rete lo fa il task di ogni istanza.
    pub fn thread_removed(&self, channel: &Channel, thread_id: &str) {
        self.notices.thread_removed(ThreadKey { channel: channel.clone(), thread_id: thread_id.to_string() });
    }

    /// Un workspace e' stato rimosso: le istanze di contatto in esecuzione eliminano le
    /// sotto-chat di tutti i suoi thread. Rapido, non fa I/O.
    pub fn channel_removed(&self, channel: &Channel) {
        self.notices.channel_removed(channel.clone());
    }

    /// Dimentica un'istanza eliminata: ferma il task, via stato di salute e codici di abbinamento.
    pub fn forget_instance(&self, id: &str) {
        self.stop_task(id);
        self.statuses.remove(id);
        self.pairing.forget(id);
        self.migration_failed().remove(id);
    }

    fn env(&self) -> Result<Arc<Env>, String> {
        self.env.get().cloned().ok_or_else(|| "connettori non inizializzati".to_string())
    }

    fn running(&self) -> MutexGuard<'_, HashMap<String, Running>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn migration_failed(&self) -> MutexGuard<'_, HashSet<String>> {
        self.migration_failed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Interrompe il task di `id` (se c'e') e toglie le sue notifiche in coda e i codici.
    fn stop_task(&self, id: &str) {
        self.statuses.end_run(id);
        let removed = self.running().remove(id);
        if let Some(running) = removed {
            running.handle.abort();
        }
        self.notices.unregister(id);
        self.pairing.forget(id);
    }

    fn migrate_all(&self) {
        let ids: Vec<String> = self.store.read(|f| f.instances.iter().map(|i| i.id.clone()).collect());
        for id in ids {
            self.migrate_one(&id);
        }
    }

    /// Migra in memoria la configurazione di un'istanza di un connettore noto.
    fn migrate_one(&self, id: &str) {
        let Some(instance) = self.store.instance(id) else { return };
        let Some(imp) = registry::find(&instance.connector) else { return };
        let mut file = self.store.snapshot();
        match migrate_instance(&mut file, id, imp.connector()) {
            Ok(true) => {
                self.store.replace(file);
                self.migration_failed().remove(id);
            }
            Ok(false) => {
                self.migration_failed().remove(id);
            }
            Err(e) => {
                log::error!("connettori {id}: migrazione della configurazione non riuscita: {e}");
                self.migration_failed().insert(id.to_string());
            }
        }
    }

    fn start(&self, env: &Arc<Env>, id: &str) {
        let Some(instance) = self.store.instance(id) else { return };
        let Some(imp) = registry::find(&instance.connector) else { return };
        if self.migration_failed().contains(id) {
            self.statuses.mark_failed(id, HealthState::Error, MIGRATION_FAILED, None);
            return;
        }
        if let Err(e) = validate_instance(imp.connector(), &instance) {
            log::error!("connettori {id}: configurazione non valida: {e}");
            self.statuses.mark_failed(id, HealthState::Error, CONFIG_INVALID, None);
            return;
        }
        let notices = match imp {
            ConnectorImpl::Contact(contact) => self.notices.register(id, contact.descriptor()),
            ConnectorImpl::Source(_) => None,
        };
        let epoch = self.statuses.begin_run(id);
        self.statuses.run_state(id, epoch, HealthState::Starting);
        let state = StateHandle::new(InstanceState::default());
        let job = Job { instance, imp, state: state.clone(), notices, epoch };
        let handle = tauri::async_runtime::spawn(run(env.clone(), job));
        self.running().insert(id.to_string(), Running { handle, state });
    }
}

#[cfg(test)]
mod tests;
