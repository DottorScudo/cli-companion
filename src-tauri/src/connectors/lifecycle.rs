//! Il runtime dei connettori come ascoltatore del ciclo di vita dell'HUB:
//! acceso avvia le istanze, spento le ferma tutte (e scarta le notifiche in
//! coda); un workspace rimosso toglie le regole del suo canale e azzera il
//! thread corrente dei collegamenti su quel canale (anche con l'HUB spento).

use super::runtime::manager::ConnectorsRuntime;
use crate::hub::{Channel, HubLifecycle};
use tauri::{AppHandle, Manager};

pub struct RuntimeLifecycle;

impl HubLifecycle for RuntimeLifecycle {
    fn hub_enabled_changed(&self, app: &AppHandle, enabled: bool) {
        app.state::<ConnectorsRuntime>().set_hub_enabled(enabled);
    }

    fn workspace_removed(&self, app: &AppHandle, ws_id: &str) {
        let Ok(channel) = Channel::parse(ws_id) else {
            log::warn!("connettori: id workspace non valido, regole non toccate");
            return;
        };
        if let Err(e) = app.state::<ConnectorsRuntime>().forget_channel(&channel) {
            log::error!("connettori: rimozione del workspace {ws_id} non salvata: {e}");
        }
    }
}
