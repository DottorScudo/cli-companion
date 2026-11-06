//! Il runtime dei connettori come ascoltatore del ciclo di vita dell'HUB:
//! acceso avvia le istanze, spento le ferma tutte (e scarta le notifiche in
//! coda); un workspace rimosso toglie le regole del suo canale e azzera il
//! thread corrente dei collegamenti su quel canale (anche con l'HUB spento) e
//! fa eliminare le sotto-chat dei suoi thread; un thread eliminato fa eliminare
//! le sue sotto-chat. Gli ascoltatori sono rapidi: la rete la usano i task delle istanze.

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
        let runtime = app.state::<ConnectorsRuntime>();
        runtime.channel_removed(&channel);
        if let Err(e) = runtime.forget_channel(&channel) {
            log::error!("connettori: rimozione del workspace {ws_id} non salvata: {e}");
        }
    }

    fn thread_removed(&self, app: &AppHandle, channel: &Channel, thread_id: &str) {
        app.state::<ConnectorsRuntime>().thread_removed(channel, thread_id);
    }
}
