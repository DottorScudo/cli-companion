//! Ciclo di vita dei pacchetti nel core: installazione, rimozione e
//! aggiornamento di quelli generati dalle integrazioni che dichiarano `package`.
//!
//! Il pacchetto e' versionato (`IntegrationPackage::version`): dopo ogni
//! installazione la versione va nel marcatore `.installed-version` della sua
//! cartella. `find_outdated` (solo letture di file) confronta il marcatore con
//! la versione attesa; `update_all` reinstalla quelli indietro.
//!
//! Operazioni sullo stesso (agente, target) non si sovrappongono: ognuna
//! rileva e lavora sotto lo stesso lock.

use super::contracts::{plugins_dir, AgentIntegration, Detection, InstallCtx, IntegrationPackage};
use super::detection::list_targets;
use super::registry;
use crate::platform::Target;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use ts_rs::TS;

/// File nella cartella del pacchetto generato con la versione installata.
pub(super) const INSTALLED_MARKER: &str = ".installed-version";

type LockKey = (String, String);

fn target_locks() -> &'static Mutex<HashMap<LockKey, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<LockKey, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(Default::default)
}

/// Esegue `f` tenendo il lock di (agente, target): installazione, rimozione e
/// aggiornamento dello stesso pacchetto sono serializzati, quelli di coppie
/// diverse no. Un lock avvelenato da un panic non blocca le operazioni dopo.
pub(super) fn with_target_lock<T>(agent_id: &str, target: &Target, f: impl FnOnce() -> T) -> T {
    let lock = {
        let mut locks = target_locks().lock().unwrap_or_else(|e| e.into_inner());
        locks.entry((agent_id.to_string(), target.key())).or_default().clone()
    };
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f()
}

fn package_of(integration: &dyn AgentIntegration) -> Result<&dyn IntegrationPackage, String> {
    integration
        .capabilities()
        .package
        .ok_or_else(|| format!("{} non ha un pacchetto da installare", integration.display_name()))
}

/// Installa il pacchetto e annota nella sua cartella la versione installata.
pub(super) fn install_and_mark(
    integration: &dyn AgentIntegration,
    ctx: &InstallCtx,
    target: &Target,
    det: &Detection,
) -> Result<String, String> {
    let package = package_of(integration)?;
    let log = package.install(ctx, target, det)?;
    if let Some(v) = package.version(ctx, target) {
        let marker = ctx.plugin_root(integration.id(), target).join(INSTALLED_MARKER);
        std::fs::write(&marker, v).map_err(|e| format!("marcatore versione: {e}\n{log}"))?;
    }
    Ok(log)
}

/// Risolve agente + target (la distro deve esistere).
fn resolve(agent_id: &str, target_key: &str) -> Result<(&'static dyn AgentIntegration, Target), String> {
    let integration = registry::find(agent_id)?;
    let target = Target::from_key(target_key)?;
    if let Target::Wsl { distro: d } = &target {
        if !list_targets().contains(&target) {
            return Err(format!("distro non trovata: {d}"));
        }
    }
    Ok((integration, target))
}

/// Installa il pacchetto dell'agente nel target; ritorna il log dei passi.
pub(super) fn install(ctx: &InstallCtx, agent_id: &str, target_key: &str) -> Result<String, String> {
    let (integration, target) = resolve(agent_id, target_key)?;
    with_target_lock(agent_id, &target, || {
        let det = integration.detect(&target);
        if !det.found {
            return Err(format!("{} non trovato in questo env: installalo prima", integration.display_name()));
        }
        install_and_mark(integration, ctx, &target, &det)
    })
}

/// Rimuove il pacchetto dell'agente dal target; ritorna il log dei passi.
pub(super) fn remove(ctx: &InstallCtx, agent_id: &str, target_key: &str) -> Result<String, String> {
    let (integration, target) = resolve(agent_id, target_key)?;
    with_target_lock(agent_id, &target, || {
        let det = integration.detect(&target);
        package_of(integration)?.uninstall(ctx, &target, &det)
    })
}

/// Versione annotata nella cartella `root` di un pacchetto generato; `None` se
/// il marcatore manca o e' vuoto (installato prima del marcatore, o non installato).
pub(super) fn installed_version(root: &Path) -> Option<String> {
    let version = std::fs::read_to_string(root.join(INSTALLED_MARKER)).ok()?;
    Some(version.trim().to_string()).filter(|v| !v.is_empty())
}

/// L'integrazione dichiara un pacchetto e ne esiste una copia installata in
/// almeno un target (una sottocartella di `agent-plugins/<id>` con il
/// marcatore). Solo letture di file, nessun processo: serve a capire quali
/// integrazioni l'utente usava gia'.
pub(super) fn installed_somewhere(data_dir: &Path, integration: &dyn AgentIntegration) -> bool {
    if integration.capabilities().package.is_none() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(plugins_dir(data_dir, integration.id())) else { return false };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|path| path.is_dir() && installed_version(&path).is_some())
}

/// Pacchetto installato con una versione diversa da quella che l'app genererebbe ora.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct OutdatedPlugin {
    pub agent_id: String,
    pub agent_name: String,
    pub env_key: String,
    pub env_label: String,
    /// `None` = installato prima del marcatore di versione.
    pub installed_version: Option<String>,
    pub expected_version: String,
}

/// Pacchetti installati (cartella presente) la cui versione non e' quella
/// attesa. Solo letture di file: `targets` (costoso, WSL) e' chiamato solo se
/// esiste una cartella diversa da "windows", e al massimo una volta.
pub(super) fn find_outdated(
    integrations: &[&dyn AgentIntegration],
    ctx: &InstallCtx,
    targets: &mut dyn FnMut() -> Vec<Target>,
) -> Vec<OutdatedPlugin> {
    let mut all: Option<Vec<Target>> = None;
    let mut out = Vec::new();
    for integration in integrations {
        let Some(package) = integration.capabilities().package else { continue };
        let base = plugins_dir(&ctx.data_dir, integration.id());
        let Ok(entries) = std::fs::read_dir(&base) else { continue };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in names {
            let target = if name == Target::Windows.key() {
                Target::Windows
            } else {
                let list = all.get_or_insert_with(&mut *targets);
                // Distro rimossa: nessun target da aggiornare.
                match list.iter().find(|t| super::sanitize_env_key(&t.key()) == name) {
                    Some(t) => t.clone(),
                    None => continue,
                }
            };
            let Some(expected) = package.version(ctx, &target) else { continue };
            let installed = installed_version(&base.join(&name));
            if installed.as_deref() != Some(expected.as_str()) {
                out.push(OutdatedPlugin {
                    agent_id: integration.id().to_string(),
                    agent_name: integration.display_name().to_string(),
                    env_key: target.key(),
                    env_label: target.label(),
                    installed_version: installed,
                    expected_version: expected,
                });
            }
        }
    }
    out
}

/// Esito dell'aggiornamento di un pacchetto.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PluginUpdateResult {
    pub agent_id: String,
    pub agent_name: String,
    pub env_key: String,
    pub env_label: String,
    pub ok: bool,
    /// Log dell'installazione, o il testo dell'errore.
    pub log: String,
}

/// Reinstalla un pacchetto non aggiornato.
fn update_one(ctx: &InstallCtx, o: OutdatedPlugin) -> PluginUpdateResult {
    let res = (|| {
        let integration = registry::find(&o.agent_id)?;
        let target = Target::from_key(&o.env_key)?;
        with_target_lock(&o.agent_id, &target, || {
            let det = integration.detect(&target);
            if !det.found {
                return Err(format!("{} non trovato in questo env", integration.display_name()));
            }
            install_and_mark(integration, ctx, &target, &det)
        })
    })();
    PluginUpdateResult {
        agent_id: o.agent_id,
        agent_name: o.agent_name,
        env_key: o.env_key,
        env_label: o.env_label,
        ok: res.is_ok(),
        log: res.unwrap_or_else(|e| e),
    }
}

/// Reinstalla i pacchetti non aggiornati delle `integrations`, in parallelo.
pub(super) fn update_all(integrations: &[&dyn AgentIntegration], ctx: &InstallCtx) -> Vec<PluginUpdateResult> {
    let outdated = find_outdated(integrations, ctx, &mut list_targets);
    std::thread::scope(|s| {
        let handles: Vec<_> = outdated.into_iter().map(|o| s.spawn(move || update_one(ctx, o))).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn outdated_detection_reads_markers() {
        let dir = temp_dir("outdated");
        let base = dir.join("agent-plugins").join("versioned");
        for (folder, marker) in
            [("windows", None), ("Ubuntu", Some("1.0-habc\n")), ("Debian", Some("0.9-hold")), ("Gone", None)]
        {
            std::fs::create_dir_all(base.join(folder)).unwrap();
            if let Some(m) = marker {
                std::fs::write(base.join(folder).join(INSTALLED_MARKER), m).unwrap();
            }
        }
        let ctx = InstallCtx { exe: "C:/a.exe".into(), data_dir: dir.clone() };
        let mut calls = 0;
        let mut targets = || {
            calls += 1;
            vec![Target::Windows, Target::Wsl { distro: "Ubuntu".into() }, Target::Wsl { distro: "Debian".into() }]
        };
        let versioned = FakeIntegration::versioned("versioned", "1.0-habc");
        let out = find_outdated(&[&versioned, &FakeIntegration::bare("none", "none-cli")], &ctx, &mut targets);
        let found: Vec<_> = out.iter().map(|o| (o.env_key.as_str(), o.installed_version.as_deref())).collect();
        // Marcatore mancante => da aggiornare; uguale => no; vecchio => si'; distro ignota => saltata.
        assert_eq!(found, vec![("Debian", Some("0.9-hold")), ("windows", None)]);
        assert!(out.iter().all(|o| o.expected_version == "1.0-habc" && o.agent_id == "versioned"));
        assert_eq!(calls, 1);

        // Solo "windows": la lista dei target non viene nemmeno calcolata.
        std::fs::remove_dir_all(base.join("Ubuntu")).unwrap();
        std::fs::remove_dir_all(base.join("Debian")).unwrap();
        std::fs::remove_dir_all(base.join("Gone")).unwrap();
        let mut never = || -> Vec<Target> { panic!("list_targets non deve essere chiamata") };
        assert_eq!(find_outdated(&[&versioned], &ctx, &mut never).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installed_version_reads_a_trimmed_non_empty_marker() {
        let dir = temp_dir("marker");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(installed_version(&dir), None);
        std::fs::write(dir.join(INSTALLED_MARKER), " \n").unwrap();
        assert_eq!(installed_version(&dir), None);
        std::fs::write(dir.join(INSTALLED_MARKER), "1.0-habc\n").unwrap();
        assert_eq!(installed_version(&dir).as_deref(), Some("1.0-habc"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installed_somewhere_needs_a_package_and_a_marker_in_some_target() {
        let dir = temp_dir("somewhere");
        let versioned = FakeIntegration::versioned("somewhere", "1.0");
        let bare = FakeIntegration::bare("somewhere-bare", "bare-cli");
        assert!(!installed_somewhere(&dir, &versioned));
        // Cartelle senza marcatore (o con marcatore vuoto) non contano.
        let base = dir.join("agent-plugins").join("somewhere");
        std::fs::create_dir_all(base.join("windows")).unwrap();
        std::fs::create_dir_all(base.join("Ubuntu")).unwrap();
        std::fs::write(base.join("Ubuntu").join(INSTALLED_MARKER), "").unwrap();
        assert!(!installed_somewhere(&dir, &versioned));
        std::fs::write(base.join("windows").join(INSTALLED_MARKER), "1.0").unwrap();
        assert!(installed_somewhere(&dir, &versioned));
        // Senza il contratto package non e' mai installata, qualunque cosa ci sia su disco.
        let bare_base = dir.join("agent-plugins").join("somewhere-bare").join("windows");
        std::fs::create_dir_all(&bare_base).unwrap();
        std::fs::write(bare_base.join(INSTALLED_MARKER), "1.0").unwrap();
        assert!(!installed_somewhere(&dir, &bare));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_and_mark_writes_the_expected_version() {
        let dir = temp_dir("mark");
        let ctx = InstallCtx { exe: "C:/a.exe".into(), data_dir: dir.clone() };
        let fake = FakeIntegration::versioned("marked", "2.0-hxyz");
        let root = ctx.plugin_root("marked", &Target::Windows);
        std::fs::create_dir_all(&root).unwrap();
        install_and_mark(&fake, &ctx, &Target::Windows, &Detection::default()).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(INSTALLED_MARKER)).unwrap(), "2.0-hxyz");
        // Senza il contratto non si installa nulla.
        let bare = FakeIntegration::bare("bare", "bare-cli");
        assert!(install_and_mark(&bare, &ctx, &Target::Windows, &Detection::default()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_agent_and_target_never_overlap() {
        let inside = AtomicUsize::new(0);
        let max_inside = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..50 {
                        with_target_lock("lock-test-same", &Target::Windows, || {
                            let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                            max_inside.fetch_max(now, Ordering::SeqCst);
                            std::thread::yield_now();
                            inside.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                });
            }
        });
        assert_eq!(max_inside.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn different_targets_do_not_block_each_other() {
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (other_tx, other_rx) = mpsc::channel();
        std::thread::scope(|s| {
            // Tiene il lock di (agente, windows) finche' non riceve il via.
            s.spawn(move || {
                with_target_lock("lock-test-diff", &Target::Windows, || {
                    held_tx.send(()).unwrap();
                    let _ = release_rx.recv_timeout(Duration::from_secs(10));
                });
            });
            held_rx.recv_timeout(Duration::from_secs(10)).expect("lock A preso");
            // Altro target e altro agente: devono entrare mentre A e' tenuto.
            let wsl = Target::Wsl { distro: "Ubuntu".into() };
            s.spawn(move || {
                with_target_lock("lock-test-diff", &wsl, || other_tx.send("target").unwrap());
                with_target_lock("lock-test-diff-other", &Target::Windows, || other_tx.send("agent").unwrap());
            });
            let first = other_rx.recv_timeout(Duration::from_secs(5));
            let second = other_rx.recv_timeout(Duration::from_secs(5));
            release_tx.send(()).unwrap();
            assert_eq!((first, second), (Ok("target"), Ok("agent")));
        });
    }

    #[test]
    fn a_panic_inside_the_lock_does_not_poison_later_operations() {
        let crashed = std::thread::spawn(|| with_target_lock("lock-test-poison", &Target::Windows, || panic!("boom")));
        assert!(crashed.join().is_err());
        assert_eq!(with_target_lock("lock-test-poison", &Target::Windows, || 7), 7);
    }
}
