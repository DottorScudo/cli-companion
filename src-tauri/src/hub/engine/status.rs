//! Lo stato dei segretari verso il frontend: per canale e thread, chi e'
//! sveglio, quando scade la sua sessione e se sta lavorando ("sta scrivendo").
//! L'evento `hub-secretary-status` porta l'elenco completo (e' piccolo: una
//! riga per sessione aperta) a ogni cambiamento; `hub_secretary_status` lo
//! legge all'apertura della vista. I messaggi dei segretari passano dal
//! normale `hub-update`.

use super::runtime::SecretaryEngine;
use super::sessions::Awake;
use crate::hub::channel::Channel;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

pub const HUB_SECRETARY_STATUS_EVENT: &str = "hub-secretary-status";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "HubSecretaryStatus")]
pub struct SecretaryStatus {
    #[ts(type = "string")]
    pub channel: Channel,
    pub thread_id: String,
    pub secretary: String,
    /// Quando la sessione si chiude se non succede nulla (millisecondi epoch);
    /// mentre lavora il conto riparte alla fine del turno.
    pub expires_at: i64,
    /// Sta lavorando sul thread o ha un turno in coda.
    pub working: bool,
}

impl From<Awake> for SecretaryStatus {
    fn from(a: Awake) -> Self {
        Self {
            channel: a.key.channel,
            thread_id: a.key.thread_id,
            secretary: a.secretary,
            expires_at: a.expires_at,
            working: a.working,
        }
    }
}

pub fn emit(app: &AppHandle, list: &[SecretaryStatus]) {
    if let Err(e) = app.emit(HUB_SECRETARY_STATUS_EVENT, list) {
        log::error!("hub: evento {HUB_SECRETARY_STATUS_EVENT} non inviato: {e}");
    }
}

/// I segretari svegli adesso (vuoto con l'HUB spento).
#[tauri::command]
pub async fn hub_secretary_status(app: AppHandle) -> Result<Vec<SecretaryStatus>, String> {
    Ok(app.state::<SecretaryEngine>().status(&app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::sessions::ThreadKey;

    #[test]
    fn the_status_is_flat_and_uses_the_channel_string() {
        let awake = Awake {
            key: ThreadKey::new(&Channel::General, "g1"),
            secretary: "mario".into(),
            expires_at: 42,
            working: true,
        };
        let json = serde_json::to_value(SecretaryStatus::from(awake)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"channel":"general","thread_id":"g1","secretary":"mario","expires_at":42,"working":true})
        );
    }
}
