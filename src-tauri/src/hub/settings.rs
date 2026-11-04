//! Impostazioni globali dell'Agents HUB (`hub_settings.json`): l'interruttore
//! (spento di default: finche' resta spento l'HUB non crea file e non cambia
//! nulla nel resto dell'app), il provider del modello, la lingua delle risposte,
//! i parametri delle sessioni e i segretari. Stesso schema di `bots.rs`: un
//! `JsonFile` statico e un `JsonStore` gestito da Tauri. Le impostazioni si
//! leggono e scrivono anche con l'HUB spento. La API key non e' qui: sta nel
//! portachiavi (`hub::api_key`).
//!
//! Un file per responsabilita':
//! - `provider`, `language`, `session`: i campi e la loro validazione;
//! - `secretary`: un segretario e la validazione dei suoi campi;
//! - `secretaries`: le operazioni pure sull'elenco;
//! - `tolerant`: la lettura che non perde il file per un valore sconosciuto;
//! - `commands`: i comandi Tauri.

pub mod commands;
mod language;
mod provider;
mod secretaries;
mod secretary;
mod session;
mod tolerant;

pub use language::Language;
pub use provider::Provider;
#[cfg(test)]
pub use secretary::SecretaryChannels;
pub use secretary::{normalize_name, Secretary, SecretaryTool};
pub use session::SessionSettings;

use crate::store::{adopt_envelope, data_dir, JsonFile, JsonStore};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

/// `#[serde(default)]` sull'intera struttura: i file scritti prima di un campo
/// nuovo (quelli della tappa 1 hanno solo `enabled`) restano validi senza
/// migrazione. Lingua e segretari si leggono in modo tollerante: un valore di
/// una versione futura non rende illeggibile il file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct HubSettings {
    pub enabled: bool,
    pub provider: Provider,
    #[serde(deserialize_with = "tolerant::or_default")]
    pub language: Language,
    pub session: SessionSettings,
    /// Al massimo `secretaries::SECRETARIES_MAX`, nomi unici.
    #[serde(deserialize_with = "tolerant::known_items")]
    pub secretaries: Vec<Secretary>,
}

/// Le impostazioni per il frontend: quelle salvate piu' la sola presenza della
/// API key (mai la chiave).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct HubSettingsView {
    #[serde(flatten)]
    pub settings: HubSettings,
    pub api_key_set: bool,
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

#[cfg(test)]
mod tests {
    use super::secretary::{SecretaryChannels, SecretaryTool};
    use super::*;
    use crate::bots::BotImage;
    use crate::hub::channel::Channel;

    fn full() -> HubSettings {
        HubSettings {
            enabled: true,
            provider: Provider::new("http://127.0.0.1:8080/v1", "qwen").unwrap(),
            language: Language::En,
            session: SessionSettings::new(30, 5).unwrap(),
            secretaries: vec![Secretary {
                name: "mario".into(),
                image: Some(BotImage { png: "AAAA".into(), color: "#112233".into() }),
                system_prompt: "Report the board status.".into(),
                tools: vec![SecretaryTool::BoardStatus, SecretaryTool::PostMessage],
                channels: SecretaryChannels::Only { channels: vec![Channel::General] },
            }],
        }
    }

    #[test]
    fn the_hub_is_off_by_default() {
        let d = HubSettings::default();
        assert!(!d.enabled);
        assert_eq!(d.provider, Provider::default());
        assert_eq!(d.language, Language::It);
        assert_eq!(d.session, SessionSettings::default());
        assert!(d.secretaries.is_empty());
        assert_eq!(SETTINGS_FILE.parse(r#"{"schema_version":1,"data":{}}"#).unwrap(), d);
    }

    #[test]
    fn stage_one_files_keep_enabled_and_get_the_new_defaults() {
        let s = SETTINGS_FILE.parse(r#"{"schema_version":1,"data":{"enabled":true}}"#).unwrap();
        assert_eq!(s, HubSettings { enabled: true, ..HubSettings::default() });
        assert!(SETTINGS_FILE.parse(r#"{"enabled":true}"#).unwrap().enabled);
    }

    #[test]
    fn serde_roundtrip_keeps_every_field() {
        let s = full();
        let raw = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<HubSettings>(&raw).unwrap(), s);
    }

    #[test]
    fn unknown_values_of_a_future_version_keep_the_rest() {
        let future = r#"{"schema_version":1,"data":{"enabled":true,"language":"pt","future_field":1,
            "session":{"idle_minutes":5,"max_steps":2,"budget":9},
            "secretaries":[{"name":"mario","tools":["board_status","decide_proposal"]},{"bad":true}]}}"#;
        let s = SETTINGS_FILE.parse(future).unwrap();
        assert!(s.enabled);
        assert_eq!(s.language, Language::It);
        assert_eq!(s.session, SessionSettings::new(5, 2).unwrap());
        assert_eq!(s.secretaries.len(), 1);
        assert_eq!(s.secretaries[0].tools, vec![SecretaryTool::BoardStatus]);
    }

    #[test]
    fn the_view_flattens_the_settings_and_has_no_key() {
        let view = HubSettingsView { settings: full(), api_key_set: true };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["api_key_set"], true);
        assert_eq!(json["provider"]["model"], "qwen");
        assert_eq!(json["secretaries"][0]["name"], "mario");
        assert!(json.get("api_key").is_none() && json.get("settings").is_none());
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
