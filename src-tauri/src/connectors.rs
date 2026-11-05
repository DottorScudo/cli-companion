//! Connettori dell'Agents HUB: moduli compilati dietro un contratto che portano
//! dati esterni nei thread (le `source`, es. una casella di posta) o permettono
//! di parlare con i segretari da un servizio di messaggistica (i `contact`, es.
//! Telegram). Progetto e regole: docs/CONNECTORS.MD.
//!
//! Contratti, descrittore e validazione, registro, file su disco, stato limitato,
//! segreti per istanza, client HTTP con allowlist degli host e il runtime: un
//! supervisore per istanza (fonti e canali di contatto), salute, abbinamento,
//! notifiche in uscita e il ciclo di vita legato all'HUB. I comandi Tauri arrivano
//! nella fase successiva.
//!
//! Dipendenze: `hub` (la porta di consegna, i tipi pubblici del modello), `store`
//! (persistenza), `secrets` (portachiavi), `validate`; il crate `reqwest` solo
//! in `runtime/http.rs`. `hub` non dipende da `connectors`.
//!
//! Un file per responsabilita':
//! - `contracts`: i trait dei connettori, gli errori, i DTO scambiati e il contesto;
//! - `descriptor`: il descrittore statico di un connettore e la sua validazione;
//! - `registry`: l'unico punto che nomina i connettori compilati;
//! - `model`: istanze, regole, collegamenti e il file `connectors.json`;
//! - `ids`, `limits`: gli id generati e i tetti;
//! - `files`: `connectors.json` e `connector_state/<istanza>.json` su `store/`;
//! - `state`: lo stato di un'istanza (cursori, id visti, mappe), limitato;
//! - `secrets`: i segreti di un'istanza nel portachiavi;
//! - `runtime`: la parte che esegue (supervisori, salute, abbinamento, notifiche);
//! - `lifecycle`: il runtime come ascoltatore del ciclo di vita dell'HUB.
//!
//! Come aggiungere un connettore: una cartella `connectors/<nome>/` che implementa
//! `SourceConnector` o `ContactConnector` e una riga in `registry.rs`; i test
//! `conformance.rs` e `architecture.rs` lo verificano da soli.

#[cfg(test)]
mod architecture;
pub mod commands;
#[cfg(test)]
mod conformance;
pub mod contracts;
pub mod descriptor;
pub mod files;
pub mod ids;
mod lifecycle;
pub mod limits;
pub mod model;
pub mod registry;
pub mod runtime;
pub mod secrets;
pub mod state;
#[cfg(test)]
pub(crate) mod testing;

use crate::hub;
use runtime::manager::ConnectorsRuntime;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Carica `connectors.json` e prepara il runtime. Non avvia nulla.
pub fn load_from_disk(app: &AppHandle) {
    app.state::<ConnectorsRuntime>().load(app);
}

/// Collega il runtime all'HUB (ciclo di vita e notifiche) e avvia le istanze se
/// l'HUB e' acceso. Va chiamata una volta, in `setup`, dopo `load_from_disk`.
pub fn start(app: &AppHandle) {
    let runtime = app.state::<ConnectorsRuntime>();
    hub::set_lifecycle_listener(app, Arc::new(lifecycle::RuntimeLifecycle));
    hub::set_notice_sink(app, runtime.notice_sink());
    runtime.set_hub_enabled(hub::settings::is_enabled(app));
}

/// Ferma tutti i supervisori (uscita dall'app).
pub fn stop_all(app: &AppHandle) {
    app.state::<ConnectorsRuntime>().stop_all();
}
