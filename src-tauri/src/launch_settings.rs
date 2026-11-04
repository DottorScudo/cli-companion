//! Impostazioni globali degli avvii: l'avvio predefinito dei nuovi terminali
//! quando il workspace non ne ha uno suo, e il flag globale che abilita la
//! lettura dei consumi dell'agente (`show_usage`). Persistenza in
//! `launch_settings.json` tramite `store` (busta versionata, migrazioni).

use crate::store::{data_dir, JsonFile, JsonStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LaunchSettings {
    /// Id di un avvio integrato (`builtin:...`); `None` = shell semplice.
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

/// Passo 0 -> 1: introduce la busta e migra l'avvio predefinito
/// `builtin:orchestrator` (v0.11) in `builtin:preset:task-intake`.
fn migrate_v0(mut v: Value) -> Result<Value, String> {
    if let Some(slot) = v.get_mut("default_launch") {
        let mut id = slot.as_str().map(str::to_string);
        crate::agents::presets::migrate_launch_id(&mut id);
        if let Some(id) = id {
            *slot = Value::String(id);
        }
    }
    Ok(v)
}

static SETTINGS_FILE: JsonFile<LaunchSettings> = JsonFile::new("launch_settings.json", &[migrate_v0]);

pub type LaunchSettingsStore = JsonStore<LaunchSettings>;

pub fn new_store() -> LaunchSettingsStore {
    JsonStore::new(&SETTINGS_FILE)
}

/// Solo `None` o l'id di un avvio integrato (quelli personalizzati sono per workspace).
fn validate(launch_id: &Option<String>, builtin: &[crate::agents::BuiltinLaunch]) -> Result<(), String> {
    match launch_id {
        Some(id) if !builtin.iter().any(|b| b.id == *id) => Err("avvio predefinito non valido".to_string()),
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
        validate(&launch_id, &crate::agents::builtin_launch_list())?;
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
    fn only_none_or_builtin_ids_are_valid() {
        let builtin = crate::agents::builtin_launch_list();
        assert!(validate(&None, &builtin).is_ok());
        assert!(validate(&Some("builtin:claude-code".into()), &builtin).is_ok());
        assert!(validate(&Some("builtin:preset:task-intake".into()), &builtin).is_ok());
        assert!(validate(&Some("builtin:orchestrator".into()), &builtin).is_err());
        assert!(validate(&Some("shell".into()), &builtin).is_err());
        assert!(validate(&Some("ln-1-1".into()), &builtin).is_err());
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
    fn v0_orchestrator_becomes_task_intake() {
        let (dir, store) =
            load_fixture("v0-orchestrator", r#"{"default_launch":"builtin:orchestrator","show_usage":false}"#);
        assert_eq!(store.get().default_launch.as_deref(), Some("builtin:preset:task-intake"));
        assert!(!store.get().show_usage);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_only_touches_a_string_default_launch() {
        use serde_json::json;
        assert_eq!(migrate_v0(json!({"default_launch": null})).unwrap(), json!({"default_launch": null}));
        assert_eq!(migrate_v0(json!({"show_usage": false})).unwrap(), json!({"show_usage": false}));
        assert_eq!(migrate_v0(json!([1])).unwrap(), json!([1]));
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
        assert_eq!(raw["schema_version"], 1);
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
        static LOCAL_FILE: JsonFile<LaunchSettings> = JsonFile::new("launch_settings.json", &[migrate_v0]);
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
