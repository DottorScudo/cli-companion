//! Il supervisore di un'istanza: un task asincrono che fa girare il connettore
//! finche' l'utente non lo ferma (disabilita, elimina, HUB spento, uscita
//! dall'app: il manager lo interrompe con `abort`). Qui l'ambiente comune, il
//! ritmo degli errori (`Pace`) e il ciclo delle fonti; il ciclo dei canali di
//! contatto e' in `contact_loop`.
//!
//! Stati: `starting` -> `ok` | `backoff` | `needs_auth` | `error`. Un errore
//! transitorio o `RateLimited` fa aspettare (backoff) e riprovare; `Auth` porta a
//! `needs_auth` e `Config`/`Fatal` a `error`, e il task finisce: riparte solo con
//! `restart_instance`. Ogni chiamata al connettore ha il suo timeout (30 s), un
//! giro intero 120 s. Generico su `HubPort`, `Clock` e `Sleeper`: i test girano
//! senza rete, senza AppHandle e senza aspettare.

use crate::connectors::contracts::{
    Clock, ConnectorContext, ConnectorError, ConnectorImpl, ContextParts, HubPort, SourceConnector, StateHandle,
};
use crate::connectors::descriptor::ConnectorDescriptor;
use crate::connectors::ids::IdGenerator;
use crate::connectors::limits::{ITEMS_PER_ROUND, MIN_INTERVAL, ROUND_TIMEOUT};
use crate::connectors::model::{ConnectorInstance, THREAD_GONE};
use crate::connectors::runtime::backoff::{jitter_from, Backoff};
use crate::connectors::runtime::contact::{ContactCtx, ContactSession};
use crate::connectors::runtime::contact_loop::run_contact;
use crate::connectors::runtime::health::{HealthState, StatusRegistry};
use crate::connectors::runtime::http::ContextHttp;
use crate::connectors::runtime::pairing::PairingRegistry;
use crate::connectors::runtime::round::{run_round, RoundCtx, RoundEnd, RoundResult};
use crate::connectors::runtime::store::{update_blocking, ConnectorsStore, StateStore};
use crate::connectors::runtime::time::Sleeper;
use crate::connectors::secrets::InstanceSecrets;
use crate::hub::ContactNotice;
use crate::secrets::SecretStore;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

/// Tutto cio' che un supervisore usa dal mondo esterno.
pub struct Env {
    pub hub: Arc<dyn HubPort>,
    pub clock: Arc<dyn Clock>,
    pub sleeper: Arc<dyn Sleeper>,
    pub store: Arc<ConnectorsStore>,
    pub states: Arc<dyn StateStore>,
    pub statuses: Arc<StatusRegistry>,
    pub secrets: Arc<dyn SecretStore>,
    /// Il client HTTP condiviso, creato alla prima necessita' (costruirlo costa e a molti avvii non serve).
    pub client: OnceLock<Result<reqwest::Client, String>>,
    pub pairing: Arc<PairingRegistry>,
    pub ids: IdGenerator,
}

/// Un'istanza pronta a partire: gia' migrata e validata dal manager.
pub struct Job {
    pub instance: ConnectorInstance,
    pub imp: ConnectorImpl,
    /// Lo stato condiviso con eventuali azioni dell'interfaccia; il task lo riempie dal disco.
    pub state: StateHandle,
    /// Le notifiche dell'HUB, per i canali di contatto che le dichiarano.
    pub notices: Option<Receiver<ContactNotice>>,
    /// L'epoca di questa esecuzione (`StatusRegistry::begin_run`): solo con quella lo stato di salute si scrive.
    pub epoch: u64,
}

impl Env {
    pub fn client(&self) -> Result<reqwest::Client, String> {
        self.client.get_or_init(super::http::shared_client).clone()
    }
}

/// Il contesto di un'istanza (anche per le azioni dell'interfaccia e per i test).
pub fn build_context(
    env: &Env,
    instance: &ConnectorInstance,
    descriptor: &'static ConnectorDescriptor,
    state: StateHandle,
) -> Result<ConnectorContext, String> {
    Ok(ConnectorContext::new(ContextParts {
        descriptor,
        instance: instance.id.clone(),
        config: instance.config.clone(),
        secrets: InstanceSecrets::new(env.secrets.clone(), &instance.id)?,
        http: ContextHttp::new(env.client()?, descriptor.permissions.hosts),
        state,
        hub: env.hub.clone(),
        clock: env.clock.clone(),
    }))
}

/// L'intervallo fra due giri di una fonte: `interval_secs` della configurazione
/// se il descrittore lo dichiara, almeno `MIN_INTERVAL` (e il minimo del descrittore, se piu' stretto).
pub fn source_interval(descriptor: &ConnectorDescriptor, config: &Value) -> Duration {
    let floor = descriptor.limits.min_interval_secs.map_or(MIN_INTERVAL, |s| MIN_INTERVAL.max(Duration::from_secs(s)));
    let configured = descriptor
        .config_field("interval_secs")
        .and_then(|_| config.get("interval_secs"))
        .and_then(Value::as_u64)
        .map_or(MIN_INTERVAL, Duration::from_secs);
    configured.max(floor)
}

/// Elementi per giro: il tetto del runtime, o quello piu' stretto del descrittore.
pub fn items_per_round(descriptor: &ConnectorDescriptor) -> usize {
    descriptor.limits.items_per_round.map_or(ITEMS_PER_ROUND, |n| n.min(ITEMS_PER_ROUND)).max(1)
}

/// Il ritmo di un supervisore: backoff, stato di salute e attese.
pub struct Pace<'a> {
    env: &'a Env,
    instance: &'a str,
    epoch: u64,
    backoff: Backoff,
}

impl<'a> Pace<'a> {
    pub fn new(env: &'a Env, instance: &'a str, epoch: u64) -> Self {
        Self { env, instance, epoch, backoff: Backoff::new() }
    }

    pub fn success(&mut self) {
        self.backoff.reset();
        self.env.statuses.run_ok(self.instance, self.epoch, self.env.clock.now_ms());
    }

    /// Aspetta `duration`; `false` se il supervisore deve fermarsi.
    pub async fn wait(&self, duration: Duration) -> bool {
        self.env.sleeper.sleep(duration).await
    }

    /// Un errore: o ferma il supervisore (`false`) o aspetta il backoff e dice di riprovare (`true`).
    pub async fn failure(&mut self, error: &ConnectorError) -> bool {
        log::warn!("connettori {}: {error}", self.instance);
        if error.stops_the_supervisor() {
            mark_stopped(self.env, self.instance, self.epoch, error);
            return false;
        }
        let wait = self.backoff.failure(error, jitter_from(&*self.env.clock));
        let next = self.env.clock.now_ms().saturating_add(i64::try_from(wait.as_millis()).unwrap_or(i64::MAX));
        self.env.statuses.run_failed(
            self.instance,
            self.epoch,
            HealthState::Backoff,
            error.public_message(),
            Some(next),
        );
        self.wait(wait).await
    }
}

/// Lo stato finale di un errore che ferma il supervisore.
pub fn mark_stopped(env: &Env, instance: &str, epoch: u64, error: &ConnectorError) {
    let state = if matches!(error, ConnectorError::Auth(_)) { HealthState::NeedsAuth } else { HealthState::Error };
    env.statuses.run_failed(instance, epoch, state, error.public_message(), None);
}

/// Il task di un'istanza. Finisce da solo solo per un errore che richiede l'utente.
pub async fn run(env: Arc<Env>, job: Job) {
    let Job { instance, imp, state, notices, epoch } = job;
    env.statuses.run_state(&instance.id, epoch, HealthState::Starting);
    let loaded = env.states.load(&instance.id).await;
    *state.lock() = loaded;
    let ctx = match build_context(&env, &instance, imp.descriptor(), state.clone()) {
        Ok(ctx) => ctx,
        Err(e) => {
            log::error!("connettori {}: contesto non costruito: {e}", instance.id);
            env.statuses.run_failed(&instance.id, epoch, HealthState::Error, "Configurazione non valida.", None);
            return;
        }
    };
    let mut pace = Pace::new(&env, &instance.id, epoch);
    match imp {
        ConnectorImpl::Source(source) => source_loop(&env, &instance, source, &ctx, &state, &mut pace).await,
        ConnectorImpl::Contact(contact) => {
            let cc = ContactCtx {
                connector: contact,
                ctx: &ctx,
                hub: &*env.hub,
                state: &state,
                store: &env.store,
                pairing: &env.pairing,
                ids: &env.ids,
                clock: &*env.clock,
                session: ContactSession::default(),
            };
            run_contact(&env, &instance.id, epoch, &cc, notices, &mut pace).await;
        }
    }
}

/// Salva lo stato su disco; un errore va nel log e non ferma il giro (la dedupe
/// in memoria e quella dell'HUB coprono un salvataggio mancato).
pub async fn save_state(env: &Env, instance: &str, state: &StateHandle) {
    let snapshot = state.lock().clone();
    if let Err(e) = env.states.save(instance, &snapshot).await {
        log::error!("connettori {instance}: stato non salvato: {e}");
    }
}

async fn disable_routes(env: &Env, ids: Vec<String>) -> Result<(), String> {
    let now = env.clock.now_ms();
    update_blocking(&env.store, move |file| {
        for route in file.routes.iter_mut().filter(|r| ids.contains(&r.id)) {
            route.enabled = false;
            route.error = Some(THREAD_GONE.to_string());
            route.updated_at = now;
        }
        Ok(())
    })
    .await
    .map(|_| ())
}

async fn source_loop(
    env: &Env,
    instance: &ConnectorInstance,
    source: &'static dyn SourceConnector,
    ctx: &ConnectorContext,
    state: &StateHandle,
    pace: &mut Pace<'_>,
) {
    let descriptor = source.descriptor();
    let interval = source_interval(descriptor, &instance.config);
    let max_items = items_per_round(descriptor);
    loop {
        let routes = env.store.read(|file| file.routes_of(&instance.id).cloned().collect::<Vec<_>>());
        let rc = RoundCtx { source, ctx, hub: &*env.hub, state, routes: &routes, max_items };
        let RoundResult { end, disable } = match tokio::time::timeout(ROUND_TIMEOUT, run_round(&rc)).await {
            Ok(result) => result,
            Err(_) => RoundResult {
                end: RoundEnd::Failed(ConnectorError::Transient("giro scaduto".to_string())),
                disable: Vec::new(),
            },
        };
        if !disable.is_empty() {
            if let Err(e) = disable_routes(env, disable).await {
                let error = ConnectorError::Transient(e);
                if !pace.failure(&error).await {
                    return;
                }
                continue;
            }
        }
        match end {
            RoundEnd::Retry => {}
            RoundEnd::Done { more } => {
                save_state(env, &instance.id, state).await;
                pace.success();
                if !more && !pace.wait(interval).await {
                    return;
                }
            }
            RoundEnd::Failed(error) => {
                if !pace.failure(&error).await {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
