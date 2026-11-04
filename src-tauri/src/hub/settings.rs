//! Impostazione globale dell'Agents HUB (`hub_settings.json`). Spento di
//! default: finche' resta spento l'HUB non crea file e non cambia nulla nel
//! resto dell'app. Stesso schema di `bots.rs`: un `JsonFile` statico e un
//! `JsonStore` gestito da Tauri; i comandi ritornano le impostazioni salvate.

use super::events::HubFeed;
use crate::store::{adopt_envelope, data_dir, JsonFile, JsonStore};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

/// `#[serde(default)]` sull'intera struttura: i file scritti prima di un campo
/// nuovo (provider e bot arriveranno nella tappa 2) restano validi.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct HubSettings {
    pub enabled: bool,
}

static SETTINGS_FILE: JsonFile<HubSettings> = JsonFile::new("hub_settings.json", &[adopt_envelope]);

pub type HubSettingsStore = JsonStore<HubSettings>;

pub fn new_store() -> HubSettingsStore {
    JsonStore::new(&SETTINGS_FILE)
}

/// File assente = HUB spento, e nessun file viene creato.
pub fn load_from_disk(app: &tauri::App) {
    match data_dir(app.handle()) {
        Ok(dir) => app.state::<HubSettingsStore>().load(&dir),
        Err(e) => log::error!("cartella dati non disponibile, impostazioni dell'Agents HUB non caricate: {e}"),
    }
}

pub fn is_enabled(app: &AppHandle) -> bool {
    app.state::<HubSettingsStore>().read(|s| s.enabled)
}

#[tauri::command]
pub fn hub_get_settings(state: State<HubSettingsStore>) -> HubSettings {
    state.get()
}

/// Spegnere l'HUB butta le istantanee delle board. Accenderlo (solo il
/// passaggio da spento ad acceso) fissa subito come base la vista corrente di
/// ogni board, senza messaggi ne' scritture: gia' la prima modifica successiva
/// produce i suoi eventi.
#[tauri::command]
pub async fn hub_set_enabled(app: AppHandle, enabled: bool) -> Result<HubSettings, String> {
    crate::blocking(move || {
        let dir = data_dir(&app)?;
        let mut was_enabled = false;
        let saved = app.state::<HubSettingsStore>().update(&dir, |s| {
            was_enabled = s.enabled;
            s.enabled = enabled;
            Ok(())
        })?;
        let feed = app.state::<HubFeed>();
        match (was_enabled, saved.enabled) {
            (false, true) => feed.baseline(&app),
            (_, false) => feed.reset(),
            (true, true) => {}
        }
        Ok(saved)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hub_is_off_by_default() {
        assert!(!HubSettings::default().enabled);
        assert_eq!(SETTINGS_FILE.parse(r#"{"schema_version":1,"data":{}}"#).unwrap(), HubSettings::default());
    }

    #[test]
    fn files_without_the_envelope_or_with_unknown_fields_are_read() {
        assert!(SETTINGS_FILE.parse(r#"{"enabled":true}"#).unwrap().enabled);
        let future = r#"{"schema_version":1,"data":{"enabled":true,"provider":{"base_url":"x"}}}"#;
        assert!(SETTINGS_FILE.parse(future).unwrap().enabled);
    }

    #[test]
    fn a_missing_file_creates_nothing() {
        let dir = std::env::temp_dir().join(format!("cc-hub-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = new_store();
        store.load(&dir);
        assert!(!store.get().enabled);
        assert!(!dir.exists());
    }
}
