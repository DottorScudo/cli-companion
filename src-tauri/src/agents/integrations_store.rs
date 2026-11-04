//! Persistenza di quali integrazioni sono abilitate e di quale e' il
//! predefinito (`integrations.json`, tramite `store`).
//!
//! Lo stato e' di processo (non un `State` di Tauri): il registro viene letto
//! da punti senza un `AppHandle` (sanificazione delle board, regole dei
//! worker, avvii). Prima del caricamento, e in ogni test, contiene `Default`:
//! `view` lo legge come "la prima integrazione abilitata e predefinita".

use super::contracts::AgentIntegration;
use super::enablement::{normalize_loaded, view, IntegrationsState};
use super::package::installed_somewhere;
use super::registry;
use crate::store::{adopt_envelope, data_dir, JsonFile, JsonStore};
use std::path::Path;
use std::sync::OnceLock;

static FILE: JsonFile<IntegrationsState> = JsonFile::new("integrations.json", &[adopt_envelope]);

fn new_store() -> JsonStore<IntegrationsState> {
    JsonStore::new(&FILE)
}

fn global() -> &'static JsonStore<IntegrationsState> {
    static STORE: OnceLock<JsonStore<IntegrationsState>> = OnceLock::new();
    STORE.get_or_init(new_store)
}

/// Lo stato corrente, riportato alle invarianti (almeno un'integrazione
/// abilitata, predefinito fra le abilitate).
pub(super) fn current() -> IntegrationsState {
    view(&global().get(), registry::all())
}

/// Carica `integrations.json` da `dir` in `store`. Se il file manca o non ha
/// piu' senso lo stato si ricava dai pacchetti installati (`installed`) e si
/// salva: la scrittura avviene una volta sola. Un salvataggio fallito si
/// registra e basta: lo stato grezzo in memoria viene comunque letto con `view`.
fn load_into(
    store: &JsonStore<IntegrationsState>,
    dir: &Path,
    registered: &[&dyn AgentIntegration],
    installed: impl Fn(&str) -> bool,
) {
    store.load(dir);
    let raw = store.get();
    let (normalized, warnings) = normalize_loaded(&raw, registered, installed);
    for warning in &warnings {
        log::warn!("{warning}");
    }
    if normalized == raw {
        return;
    }
    let saved = store.update(dir, |state| {
        *state = normalized;
        Ok(())
    });
    if let Err(e) = saved {
        log::warn!("integrazioni abilitate non salvate: {e}");
    }
}

/// Carica lo stato all'avvio, prima di chi lo legge (workspace, board).
pub fn load_integrations(app: &tauri::App) {
    match data_dir(app.handle()) {
        Ok(dir) => {
            let installed = |id: &str| registry::all().iter().any(|i| i.id() == id && installed_somewhere(&dir, *i));
            load_into(global(), &dir, registry::all(), installed);
        }
        Err(e) => log::error!("cartella dati non disponibile, integrazioni abilitate non caricate: {e}"),
    }
}

/// Applica `rule` allo stato globale, lo salva e solo poi lo pubblica (se la
/// regola o il salvataggio falliscono nulla cambia).
pub(super) fn update(
    dir: &Path,
    rule: impl FnOnce(&IntegrationsState, &[&dyn AgentIntegration]) -> Result<IntegrationsState, String>,
) -> Result<IntegrationsState, String> {
    update_in(global(), dir, registry::all(), rule)
}

fn update_in(
    store: &JsonStore<IntegrationsState>,
    dir: &Path,
    registered: &[&dyn AgentIntegration],
    rule: impl FnOnce(&IntegrationsState, &[&dyn AgentIntegration]) -> Result<IntegrationsState, String>,
) -> Result<IntegrationsState, String> {
    store.update(dir, |state| {
        *state = rule(state, registered)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::enablement::{set_default, set_enabled};
    use crate::agents::testing::FakeIntegration;
    use std::path::PathBuf;

    const FILE_NAME: &str = "integrations.json";
    const FAKE_ID: &str = "fake-second";

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-integrations-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn state(enabled: &[&str], default_agent: &str) -> IntegrationsState {
        IntegrationsState {
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
            default_agent: default_agent.into(),
        }
    }

    fn first_id() -> &'static str {
        registry::all()[0].id()
    }

    fn written(dir: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()).unwrap()
    }

    #[test]
    fn the_store_starts_unconfigured_and_reads_as_the_first_integration() {
        assert_eq!(new_store().get(), IntegrationsState::default());
        assert_eq!(view(&IntegrationsState::default(), registry::all()), state(&[first_id()], first_id()));
    }

    #[test]
    fn a_missing_file_is_derived_from_the_installed_packages_and_written_once() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];

        let none_installed = temp_dir("none-installed");
        let store = new_store();
        load_into(&store, &none_installed, &registered, |_| false);
        assert_eq!(store.get(), state(&[first_id()], first_id()));
        let file = written(&none_installed);
        assert_eq!(file["schema_version"], 1);
        assert_eq!(file["data"]["enabled"], serde_json::json!([first_id()]));
        assert_eq!(file["data"]["default_agent"], first_id());

        let one_installed = temp_dir("one-installed");
        let store = new_store();
        load_into(&store, &one_installed, &registered, |id| id == FAKE_ID);
        assert_eq!(store.get(), state(&[FAKE_ID], FAKE_ID));
        assert_eq!(written(&one_installed)["data"]["enabled"], serde_json::json!([FAKE_ID]));

        // Riavvio: il file esiste e comanda, anche se ora risultano installate altre.
        let reloaded = new_store();
        load_into(&reloaded, &one_installed, &registered, |_| true);
        assert_eq!(reloaded.get(), state(&[FAKE_ID], FAKE_ID));
        let _ = std::fs::remove_dir_all(&none_installed);
        let _ = std::fs::remove_dir_all(&one_installed);
    }

    #[test]
    fn unknown_ids_and_a_default_that_is_not_enabled_are_fixed_and_saved() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let dir = temp_dir("fixed");
        let raw = state(&["gone", FAKE_ID], first_id());
        new_store()
            .update(&dir, |s| {
                *s = raw;
                Ok(())
            })
            .unwrap();

        let store = new_store();
        load_into(&store, &dir, &registered, |_| true);
        assert_eq!(store.get(), state(&[FAKE_ID], FAKE_ID));
        assert_eq!(written(&dir)["data"]["enabled"], serde_json::json!([FAKE_ID]));
        assert_eq!(written(&dir)["data"]["default_agent"], FAKE_ID);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_file_is_set_aside_and_replaced_by_a_derived_state() {
        let registered = [registry::all()[0]];
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join(FILE_NAME), "{ not json").unwrap();
        let store = new_store();
        load_into(&store, &dir, &registered, |_| false);
        assert_eq!(store.get(), state(&[first_id()], first_id()));
        assert_eq!(written(&dir)["schema_version"], 1);
        let backups = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("integrations.json.bak-"));
        assert_eq!(backups.count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unwritable_file_still_reads_as_the_first_integration() {
        static LOCAL_FILE: JsonFile<IntegrationsState> = JsonFile::new("integrations.json", &[adopt_envelope]);
        let registered = [registry::all()[0]];
        let dir = temp_dir("unwritable");
        std::fs::create_dir_all(dir.join(FILE_NAME)).unwrap();
        let store = JsonStore::new(&LOCAL_FILE);
        load_into(&store, &dir, &registered, |_| false);
        assert_eq!(store.get(), IntegrationsState::default());
        assert_eq!(view(&store.get(), &registered), state(&[first_id()], first_id()));
        assert!(dir.join(FILE_NAME).is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rules_are_saved_before_they_are_published_and_survive_a_reload() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let dir = temp_dir("rules");
        let store = new_store();
        load_into(&store, &dir, &registered, |_| false);

        let enabled = update_in(&store, &dir, &registered, |s, reg| set_enabled(s, reg, FAKE_ID, true)).unwrap();
        assert_eq!(enabled, state(&[first_id(), FAKE_ID], first_id()));
        let switched = update_in(&store, &dir, &registered, |s, reg| set_default(s, reg, FAKE_ID)).unwrap();
        assert_eq!(switched.default_agent, FAKE_ID);
        // Una regola rifiutata non cambia ne' la memoria ne' il file.
        let before = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        let refused = update_in(&store, &dir, &registered, |s, reg| set_enabled(s, reg, FAKE_ID, false));
        assert!(refused.unwrap_err().contains("e' l'agente predefinito"));
        assert_eq!(store.get(), switched);
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), before);

        let reloaded = new_store();
        load_into(&reloaded, &dir, &registered, |_| false);
        assert_eq!(reloaded.get(), switched);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
