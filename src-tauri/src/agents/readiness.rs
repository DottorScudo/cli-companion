//! Quanto e' pronta un'integrazione in un target: abilitata, agente trovato,
//! pacchetto installato e aggiornato. La classificazione e' pura; il
//! sondaggio (processi) passa da una cache di breve durata.

use super::commands::install_ctx;
use super::contracts::{AgentIntegration, InstallCtx};
use super::env_status::{status_for, AgentEnvStatus};
use super::package::installed_version;
use super::registry;
use crate::platform::Target;
use crate::ttl_cache::TtlCache;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;
use ts_rs::TS;

/// Validita' dei sondaggi: abbastanza per non rifare i processi a ogni
/// apertura della pagina, poco perche' un'installazione fatta a mano si veda.
const READINESS_TTL: Duration = Duration::from_secs(30);

/// Stato di un'integrazione in un target, nell'ordine di precedenza con cui
/// `classify` lo decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Readiness {
    Ready,
    /// L'agente non c'e' nel target.
    NotFound,
    /// L'agente c'e' ma il suo pacchetto non e' installato.
    NotInstalled,
    /// Il pacchetto installato non e' la versione che l'app genererebbe ora.
    Outdated,
    /// L'utente non l'ha abilitata: nessun rilevamento.
    Disabled,
}

/// Esito del sondaggio di una coppia agente x target.
#[derive(Debug, Clone)]
pub struct EnvProbe {
    pub status: AgentEnvStatus,
    /// Il pacchetto e' installato con una versione diversa da quella attesa
    /// (marcatore mancante compreso, come per `find_outdated`).
    pub outdated: bool,
}

/// Precedenza: non abilitata, non trovata, pacchetto mancante, pacchetto
/// vecchio, pronta. `has_package`: l'integrazione dichiara un pacchetto.
pub fn classify(enabled: bool, has_package: bool, probe: &EnvProbe) -> Readiness {
    if !enabled {
        Readiness::Disabled
    } else if !probe.status.found {
        Readiness::NotFound
    } else if has_package && !probe.status.installed {
        Readiness::NotInstalled
    } else if has_package && probe.outdated {
        Readiness::Outdated
    } else {
        Readiness::Ready
    }
}

/// Il pacchetto installato nel target non ha la versione attesa. Solo letture
/// di file.
fn is_outdated(integration: &dyn AgentIntegration, target: &Target, ctx: &InstallCtx) -> bool {
    let Some(package) = integration.capabilities().package else { return false };
    let Some(expected) = package.version(ctx, target) else { return false };
    installed_version(&ctx.plugin_root(integration.id(), target)).as_deref() != Some(expected.as_str())
}

/// Rileva l'agente nel target e controlla la versione del pacchetto. Lancia
/// processi: fuori dal main thread.
pub fn probe(integration: &dyn AgentIntegration, target: &Target, ctx: &InstallCtx) -> EnvProbe {
    let status = status_for(integration, target);
    let outdated = status.installed && is_outdated(integration, target, ctx);
    EnvProbe { status, outdated }
}

type ProbeKey = (String, String);

fn probe_cache() -> &'static TtlCache<ProbeKey, EnvProbe> {
    static CACHE: OnceLock<TtlCache<ProbeKey, EnvProbe>> = OnceLock::new();
    CACHE.get_or_init(|| TtlCache::new(READINESS_TTL))
}

/// Il sondaggio della coppia, dalla cache se recente.
pub(super) fn cached_probe(integration: &dyn AgentIntegration, target: &Target, ctx: &InstallCtx) -> EnvProbe {
    let key = (integration.id().to_string(), target.key());
    if let Some(hit) = probe_cache().get(&key) {
        return hit;
    }
    let fresh = probe(integration, target, ctx);
    probe_cache().put(key, fresh.clone());
    fresh
}

/// Dimentica i sondaggi: il prossimo e' fresco. Da chiamare quando cambiano
/// gli agenti o i pacchetti installati.
pub(super) fn forget_readiness() {
    probe_cache().clear();
}

/// `readiness` con l'abilitazione decisa dal chiamante.
fn readiness_with(enabled: bool, integration: &dyn AgentIntegration, target: &Target, ctx: &InstallCtx) -> Readiness {
    if !enabled {
        return Readiness::Disabled;
    }
    let has_package = integration.capabilities().package.is_some();
    classify(true, has_package, &cached_probe(integration, target, ctx))
}

/// Prontezza dell'integrazione nel target. `ctx` serve a conoscere la versione
/// attesa del pacchetto. Se l'integrazione e' disabilitata risponde subito,
/// senza rilevare nulla; altrimenti puo' lanciare processi (se il sondaggio in
/// cache e' scaduto): fuori dal main thread.
pub fn readiness(integration: &dyn AgentIntegration, target: &Target, ctx: &InstallCtx) -> Readiness {
    readiness_with(registry::is_enabled(integration.id()), integration, target, ctx)
}

/// Prontezza dell'agente `agent_id` nel target, come la vede chi non ha le
/// integrazioni in mano (l'automazione, che conosce solo l'id salvato in un
/// task). Un id fuori dal registro non e' abilitato: `Disabled`. Se il
/// contesto di installazione non si ricava (exe o cartella dati illeggibili)
/// l'errore va nel log e la prontezza si decide dal solo rilevamento, senza
/// controllare il pacchetto: come prima che esistesse questo controllo.
/// Puo' lanciare processi (sondaggio in cache scaduto): fuori dal main thread
/// e mai sotto un lock.
pub fn agent_readiness(app: &AppHandle, agent_id: &str, target: &Target) -> Readiness {
    let Ok(integration) = registry::find(agent_id) else {
        return Readiness::Disabled;
    };
    match install_ctx(app) {
        Ok(ctx) => readiness(integration, target, &ctx),
        Err(e) => {
            log::error!("prontezza di {agent_id}: contesto di installazione non disponibile: {e}");
            detected_only(integration, target)
        }
    }
}

/// Prontezza senza il controllo del pacchetto (serve un contesto che manca).
fn detected_only(integration: &dyn AgentIntegration, target: &Target) -> Readiness {
    if !registry::is_enabled(integration.id()) {
        return Readiness::Disabled;
    }
    classify(true, false, &EnvProbe { status: status_for(integration, target), outdated: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::contracts::Capability;
    use crate::agents::package::INSTALLED_MARKER;
    use crate::agents::testing::FakeIntegration;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-readiness-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ctx_in(dir: &std::path::Path) -> InstallCtx {
        InstallCtx { exe: "C:/a.exe".into(), data_dir: dir.to_path_buf() }
    }

    fn write_marker(ctx: &InstallCtx, id: &str, version: &str) {
        let root = ctx.plugin_root(id, &Target::Windows);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(INSTALLED_MARKER), version).unwrap();
    }

    fn probe_of(found: bool, installed: bool, outdated: bool) -> EnvProbe {
        let fake = FakeIntegration::versioned("probe-of", "1.0").detected(found, installed);
        EnvProbe { status: status_for(&fake, &Target::Windows), outdated }
    }

    #[test]
    fn classify_follows_the_precedence() {
        let ready = probe_of(true, true, false);
        assert_eq!(classify(true, true, &ready), Readiness::Ready);
        assert_eq!(classify(false, true, &ready), Readiness::Disabled);
        // Disabilitata vince su tutto, anche su un agente non trovato.
        assert_eq!(classify(false, true, &probe_of(false, false, false)), Readiness::Disabled);
        assert_eq!(classify(true, true, &probe_of(false, true, true)), Readiness::NotFound);
        assert_eq!(classify(true, true, &probe_of(true, false, true)), Readiness::NotInstalled);
        assert_eq!(classify(true, true, &probe_of(true, true, true)), Readiness::Outdated);
    }

    #[test]
    fn an_integration_without_a_package_only_needs_the_agent() {
        assert_eq!(classify(true, false, &probe_of(true, false, false)), Readiness::Ready);
        assert_eq!(classify(true, false, &probe_of(true, false, true)), Readiness::Ready);
        assert_eq!(classify(true, false, &probe_of(false, false, false)), Readiness::NotFound);
    }

    #[test]
    fn readiness_serializes_kebab_case() {
        let json = |r| serde_json::to_value(r).unwrap();
        assert_eq!(json(Readiness::NotFound), "not-found");
        assert_eq!(json(Readiness::NotInstalled), "not-installed");
        assert_eq!(json(Readiness::Ready), "ready");
        assert_eq!(json(Readiness::Outdated), "outdated");
        assert_eq!(json(Readiness::Disabled), "disabled");
    }

    #[test]
    fn a_disabled_integration_is_not_probed() {
        let real = registry::all()[0];
        let ctx = ctx_in(&std::env::temp_dir().join("cc-readiness-disabled"));
        // L'agente vero non viene nemmeno rilevato: la risposta e' immediata.
        assert_eq!(readiness_with(false, real, &Target::Windows, &ctx), Readiness::Disabled);
        let fake = FakeIntegration::versioned("readiness-disabled", "1.0").detected(true, true);
        assert_eq!(readiness_with(false, &fake, &Target::Windows, &ctx), Readiness::Disabled);
        // Un id fuori dal registro non e' abilitato.
        assert_eq!(readiness(&fake, &Target::Windows, &ctx), Readiness::Disabled);
        assert!(registry::is_enabled(real.id()));
    }

    #[test]
    fn readiness_covers_not_found_not_installed_outdated_and_ready() {
        let dir = temp_dir("states");
        let ctx = ctx_in(&dir);
        let check = |fake: &FakeIntegration| readiness_with(true, fake, &Target::Windows, &ctx);
        let missing = FakeIntegration::versioned("readiness-missing", "1.0").detected(false, false);
        assert_eq!(check(&missing), Readiness::NotFound);
        let not_installed = FakeIntegration::versioned("readiness-nopkg", "1.0").detected(true, false);
        assert_eq!(check(&not_installed), Readiness::NotInstalled);
        // Marcatore mancante o di un'altra versione: da aggiornare.
        let no_marker = FakeIntegration::versioned("readiness-no-marker", "1.0").detected(true, true);
        assert_eq!(check(&no_marker), Readiness::Outdated);
        let old = FakeIntegration::versioned("readiness-old", "1.0").detected(true, true);
        write_marker(&ctx, "readiness-old", "0.9");
        assert_eq!(check(&old), Readiness::Outdated);
        let current = FakeIntegration::versioned("readiness-current", "1.0").detected(true, true);
        write_marker(&ctx, "readiness-current", "1.0\n");
        assert_eq!(check(&current), Readiness::Ready);
        // Senza pacchetto basta l'agente.
        let mut no_package = FakeIntegration::bare("readiness-agent-only", "agent-cli").detected(true, false);
        no_package.declares.retain(|c| *c != Capability::Package);
        assert_eq!(check(&no_package), Readiness::Ready);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_probe_is_cached_until_it_is_forgotten() {
        let dir = temp_dir("cache");
        let ctx = ctx_in(&dir);
        let fake = FakeIntegration::versioned("readiness-cache", "2.0").detected(true, true);
        write_marker(&ctx, "readiness-cache", "1.0");
        assert_eq!(readiness_with(true, &fake, &Target::Windows, &ctx), Readiness::Outdated);
        // Reinstallato: la cache ancora dice vecchio, finche' non viene dimenticata.
        write_marker(&ctx, "readiness-cache", "2.0");
        assert_eq!(readiness_with(true, &fake, &Target::Windows, &ctx), Readiness::Outdated);
        forget_readiness();
        assert_eq!(readiness_with(true, &fake, &Target::Windows, &ctx), Readiness::Ready);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
