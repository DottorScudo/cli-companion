//! Impostazioni globali degli avvii: l'avvio predefinito dei nuovi terminali
//! quando il workspace non ne ha uno suo, e il flag globale che abilita la
//! lettura dei consumi dell'agente (`show_usage`). Persistenza in
//! `launch_settings.json` tramite `store` (busta versionata, migrazioni).

use crate::launch_migration::migrate_launch_value;
use crate::store::{data_dir, JsonFile, JsonStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LaunchSettings {
    /// Id di un avvio integrato (`builtin:...`) o di un Bot (`bot:...`);
    /// `None` = shell semplice.
    #[serde(default)]
    pub default_launch: Option<String>,
    /// Mostra i consumi dell'agente (limiti e token). I file vecchi senza il
    /// campo valgono `true`.
    #[serde(default = "default_show_usage")]
    pub show_usage: bool,
}

fn default_show_usage() -> bool {
    true
}

impl Default for LaunchSettings {
    fn default() -> Self {
        Self { default_launch: None, show_usage: true }
    }
}

/// Campo dell'avvio predefinito nel dato.
const DEFAULT_LAUNCH_KEY: &str = "default_launch";

/// Riscrive una vecchia voce in `default_launch` (vedi `launch_migration`).
fn rewrite_default_launch(mut v: Value) -> Result<Value, String> {
    if let Some(slot) = v.get_mut(DEFAULT_LAUNCH_KEY) {
        migrate_launch_value(slot);
    }
    Ok(v)
}

/// Passo 0 -> 1: introduce la busta e migra l'avvio predefinito
/// `builtin:orchestrator` (v0.11) nel Bot che lo sostituisce.
fn migrate_v0(v: Value) -> Result<Value, String> {
    rewrite_default_launch(v)
}

/// Passo 1 -> 2: le voci dei preset (`builtin:preset:<id>[@<agente>]`)
/// diventano le voci dei loro Bot (`bot:<id>`); il Bot derivato di una voce
/// con l'agente scelto lo crea `launch_migration` all'avvio.
fn migrate_v1(v: Value) -> Result<Value, String> {
    rewrite_default_launch(v)
}

const MIGRATIONS: [crate::store::Migration; 2] = [migrate_v0, migrate_v1];

static SETTINGS_FILE: JsonFile<LaunchSettings> = JsonFile::new("launch_settings.json", &MIGRATIONS);

pub type LaunchSettingsStore = JsonStore<LaunchSettings>;

pub fn new_store() -> LaunchSettingsStore {
    JsonStore::new(&SETTINGS_FILE)
}

/// Solo `None`, l'id di un avvio integrato o la voce di un Bot abilitato
/// (`bots`); quelli personalizzati sono per workspace.
fn validate(
    launch_id: &Option<String>,
    builtin: &[crate::agents::BuiltinLaunch],
    bots: &[String],
) -> Result<(), String> {
    match launch_id {
        Some(id) if !builtin.iter().any(|b| b.id == *id) && !bots.contains(id) => {
            Err("avvio predefinito non valido".to_string())
        }
        _ => Ok(()),
    }
}

pub fn load_from_disk(app: &tauri::App) {
    match data_dir(app.handle()) {
        Ok(dir) => app.state::<LaunchSettingsStore>().load(&dir),
        Err(e) => log::error!("cartella dati non disponibile, impostazioni degli avvii non caricate: {e}"),
    }
}

#[tauri::command]
pub fn get_launch_settings(state: State<LaunchSettingsStore>) -> LaunchSettings {
    state.get()
}

#[tauri::command]
pub async fn set_global_default_launch(app: AppHandle, launch_id: Option<String>) -> Result<LaunchSettings, String> {
    crate::blocking(move || {
        validate(&launch_id, &crate::agents::builtin_launch_list(), &crate::bots::enabled_launch_ids(&app))?;
        update(&app, |s| s.default_launch = launch_id)
    })
    .await
}

#[tauri::command]
pub async fn set_show_usage(app: AppHandle, enabled: bool) -> Result<LaunchSettings, String> {
    crate::blocking(move || update(&app, |s| s.show_usage = enabled)).await
}

/// Applica `f`, salva e solo poi aggiorna la memoria (se il salvataggio fallisce
/// lo stato resta invariato).
fn update(app: &AppHandle, f: impl FnOnce(&mut LaunchSettings)) -> Result<LaunchSettings, String> {
    let dir = data_dir(app)?;
    app.state::<LaunchSettingsStore>().update(&dir, |s| {
        f(s);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const FILE_NAME: &str = "launch_settings.json";

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-launch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Scrive `raw` come file di una versione precedente e lo carica con la static vera.
    fn load_fixture(name: &str, raw: &str) -> (PathBuf, LaunchSettingsStore) {
        let dir = temp_dir(name);
        std::fs::write(dir.join(FILE_NAME), raw).unwrap();
        let store = new_store();
        store.load(&dir);
        (dir, store)
    }

    fn backups(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("launch_settings.json.bak-"))
            .collect()
    }

    #[test]
    fn defaults_and_serde() {
        assert_eq!(LaunchSettings::default().default_launch, None);
        let s: LaunchSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, LaunchSettings::default());
        let s = LaunchSettings { default_launch: Some("builtin:claude-code".into()), show_usage: false };
        let back: LaunchSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn show_usage_defaults_to_true() {
        assert!(LaunchSettings::default().show_usage);
        assert!(new_store().get().show_usage);
    }

    #[test]
    fn only_none_builtin_or_bot_ids_are_valid() {
        let builtin = crate::agents::builtin_launch_list();
        let bots = ["bot:intake".to_string()];
        let check = |id: &str| validate(&Some(id.into()), &builtin, &bots);
        assert!(validate(&None, &builtin, &bots).is_ok());
        assert!(check("builtin:claude-code").is_ok());
        assert!(check("bot:intake").is_ok());
        for bad in [
            "bot:gone",
            "builtin:preset:task-intake",
            "builtin:preset:task-intake@claude-code",
            "builtin:orchestrator",
            "shell",
            "ln-1-1",
        ] {
            assert!(check(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn v0_empty_object_loads_defaults() {
        let (dir, store) = load_fixture("v0-empty", "{}");
        assert_eq!(store.get(), LaunchSettings::default());
        assert!(backups(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v0_without_show_usage_keeps_it_enabled() {
        let (dir, store) = load_fixture("v0-launch", r#"{"default_launch":"builtin:claude-code"}"#);
        assert_eq!(store.get().default_launch.as_deref(), Some("builtin:claude-code"));
        assert!(store.get().show_usage);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v0_orchestrator_becomes_the_intake_bot() {
        let (dir, store) =
            load_fixture("v0-orchestrator", r#"{"default_launch":"builtin:orchestrator","show_usage":false}"#);
        assert_eq!(store.get().default_launch.as_deref(), Some("bot:intake"));
        assert!(!store.get().show_usage);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_preset_launches_become_bot_launches() {
        let cases = [
            ("builtin:preset:task-intake", "bot:intake"),
            ("builtin:preset:project-setup", "bot:setup"),
            ("builtin:preset:skill-wizard@opencode", "bot:skill-wizard-opencode"),
            ("builtin:claude-code", "builtin:claude-code"),
        ];
        for (i, (old, new)) in cases.into_iter().enumerate() {
            let raw = format!(r#"{{"schema_version":1,"data":{{"default_launch":"{old}","show_usage":false}}}}"#);
            let (dir, store) = load_fixture(&format!("v1-preset-{i}"), &raw);
            assert_eq!(store.get(), LaunchSettings { default_launch: Some(new.into()), show_usage: false }, "{old}");
            assert!(backups(&dir).is_empty());
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn migration_only_touches_a_string_default_launch() {
        use serde_json::json;
        for step in MIGRATIONS {
            assert_eq!(step(json!({"default_launch": null})).unwrap(), json!({"default_launch": null}));
            assert_eq!(step(json!({"show_usage": false})).unwrap(), json!({"show_usage": false}));
            assert_eq!(step(json!([1])).unwrap(), json!([1]));
        }
    }

    #[test]
    fn v1_envelope_roundtrips_through_update() {
        let dir = temp_dir("roundtrip");
        let store = new_store();
        let next = store
            .update(&dir, |s| {
                s.default_launch = Some("builtin:claude-code".into());
                s.show_usage = false;
                Ok(())
            })
            .unwrap();
        assert_eq!(next, LaunchSettings { default_launch: Some("builtin:claude-code".into()), show_usage: false });
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()).unwrap();
        assert_eq!(raw["schema_version"], 2);
        assert_eq!(raw["data"]["default_launch"], "builtin:claude-code");
        assert_eq!(raw["data"]["show_usage"], false);
        let reloaded = new_store();
        reloaded.load(&dir);
        assert_eq!(reloaded.get(), next);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_backed_up_byte_for_byte_and_not_overwritten() {
        let garbage = "{\"default_launch\": ???\u{0}\r\n";
        let (dir, store) = load_fixture("corrupt", garbage);
        assert_eq!(store.get(), LaunchSettings::default());
        let found = backups(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(std::fs::read(&found[0]).unwrap(), garbage.as_bytes());
        // Nessuna riscrittura finche' l'utente non modifica qualcosa.
        assert!(!dir.join(FILE_NAME).exists());
        store
            .update(&dir, |s| {
                s.show_usage = false;
                Ok(())
            })
            .unwrap();
        assert!(dir.join(FILE_NAME).is_file());
        assert_eq!(std::fs::read(&found[0]).unwrap(), garbage.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_file_keeps_defaults_and_refuses_updates() {
        // Istanza locale: il blocco delle scritture e' appiccicoso per istanza.
        static LOCAL_FILE: JsonFile<LaunchSettings> = JsonFile::new("launch_settings.json", &MIGRATIONS);
        let dir = temp_dir("unreadable");
        std::fs::create_dir_all(dir.join(FILE_NAME)).unwrap();
        let store = JsonStore::new(&LOCAL_FILE);
        store.load(&dir);
        assert_eq!(store.get(), LaunchSettings::default());
        let r = store.update(&dir, |s| {
            s.show_usage = false;
            Ok(())
        });
        assert!(r.is_err());
        assert_eq!(store.get(), LaunchSettings::default());
        assert!(dir.join(FILE_NAME).is_dir());
        assert!(backups(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
