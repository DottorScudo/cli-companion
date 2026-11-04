//! Prontezza degli agenti per lo scheduler. Lo scheduler decide sotto il lock
//! del registro e non puo' aspettare i processi del rilevamento: qui sta la
//! cache dell'ultimo valore noto, per (target, agente). `cached_readiness` non
//! blocca mai: risponde con il valore noto e, se e' scaduto, ne fa partire il
//! rinnovo su un thread bloccante; a rinnovo finito, se il valore e' cambiato,
//! l'automazione rivaluta le board (`recheck_agents`).
//!
//! Lock: il mutex della cache e' una foglia, mai tenuto mentre si entra nel
//! registro o si lancia un processo.

use super::*;
use crate::agents;
use crate::platform::Target;
use std::time::Instant;

/// Dopo quanto il valore noto si rinnova. Meno del giro del timer (30 s):
/// a ogni giro il valore dei task in attesa e' al piu' di un giro prima.
const REFRESH_AFTER: Duration = Duration::from_secs(15);
/// Un rinnovo che non finisce da tanto (thread morto) non blocca per sempre
/// quelli successivi.
const PROBE_STUCK_AFTER: Duration = Duration::from_secs(120);

/// (chiave del target, id dell'agente).
type Key = (String, String);

#[derive(Default)]
struct Slot {
    value: Option<Readiness>,
    checked_at: Option<Instant>,
    probing_since: Option<Instant>,
}

/// Esito di `AgentProbes::lookup`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Lookup {
    /// Ultimo valore noto (anche scaduto); `None` = mai controllato.
    pub known: Option<Readiness>,
    /// Chi chiama deve far partire il rinnovo (ora risulta in corso).
    pub start_probe: bool,
}

/// Ultimo valore noto per coppia target x agente. Parte pura: il tempo lo
/// passa chi chiama.
#[derive(Default)]
pub(super) struct AgentProbes {
    slots: Mutex<HashMap<Key, Slot>>,
}

impl AgentProbes {
    /// Valore noto e se serve un rinnovo (scaduto e nessuno in corso).
    fn lookup(&self, key: &Key, now: Instant) -> Lookup {
        let mut slots = lock(&self.slots);
        let slot = slots.entry(key.clone()).or_default();
        let age = |at: Option<Instant>, limit: Duration| at.is_some_and(|t| now.saturating_duration_since(t) < limit);
        let start_probe = !age(slot.checked_at, REFRESH_AFTER) && !age(slot.probing_since, PROBE_STUCK_AFTER);
        if start_probe {
            slot.probing_since = Some(now);
        }
        Lookup { known: slot.value, start_probe }
    }

    /// Registra un valore appena rilevato; `true` se e' diverso dal noto (o
    /// non c'era).
    fn record(&self, key: Key, value: Readiness, now: Instant) -> bool {
        let mut slots = lock(&self.slots);
        let slot = slots.entry(key).or_default();
        let changed = slot.value != Some(value);
        *slot = Slot { value: Some(value), checked_at: Some(now), probing_since: None };
        changed
    }
}

/// Dove gira l'agente del workspace. Un target illegibile (workspace WSL
/// senza distro) e' un agente irraggiungibile: `NotFound`, con l'errore nel log.
fn key_and_target(ws: &Workspace, agent: &str) -> Result<(Key, Target), Readiness> {
    let target = ws.target().map_err(|e| {
        log::warn!("workspace {}: target non valido, agente {agent} non raggiungibile: {e}", ws.id);
        Readiness::NotFound
    })?;
    Ok(((target.key(), agent.to_string()), target))
}

/// Ultimo valore noto, senza bloccare; il rinnovo parte da solo se serve. Sicuro
/// sotto il lock del registro.
pub(super) fn cached_readiness(app: &AppHandle, ws: &Workspace, agent: &str) -> Option<Readiness> {
    let (key, target) = match key_and_target(ws, agent) {
        Ok(found) => found,
        Err(unreachable) => return Some(unreachable),
    };
    let st = app.state::<AutomationState>();
    let lookup = st.probes.lookup(&key, Instant::now());
    if lookup.start_probe {
        let app = app.clone();
        let agent = agent.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let readiness = agents::agent_readiness(&app, &agent, &target);
            store(&app, key, readiness);
        });
    }
    lookup.known
}

/// Valore fresco (passa dalla cache dei sondaggi dell'integrazione, 30 s), e
/// aggiorna quello noto. Bloccante: mai sotto un lock.
pub(super) fn fresh_readiness(app: &AppHandle, ws: &Workspace, agent: &str) -> Readiness {
    let (key, target) = match key_and_target(ws, agent) {
        Ok(found) => found,
        Err(unreachable) => return unreachable,
    };
    let readiness = agents::agent_readiness(app, agent, &target);
    store(app, key, readiness);
    readiness
}

/// Memorizza il valore e, se e' cambiato, fa rivalutare le board.
fn store(app: &AppHandle, key: Key, readiness: Readiness) {
    let st = app.state::<AutomationState>().inner().clone();
    if st.probes.record(key, readiness, Instant::now()) {
        st.recheck_agents(&TauriHost(app.clone()));
    }
}

#[cfg(test)]
mod tests;
