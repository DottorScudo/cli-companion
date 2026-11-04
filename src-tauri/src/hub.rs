//! Agents HUB: modalita' opzionale, spenta di default, con un canale per
//! workspace piu' il Generale e, dentro ogni canale, thread di messaggi.
//! Questa tappa (fondamenta) porta l'impostazione globale, il modello dati, la
//! persistenza, i comandi Tauri e l'evento `hub-update`; eventi della board,
//! bot e UI arrivano dopo. Documentazione: docs/HUB.MD.
//!
//! Dipendenze: `store` (persistenza), `board` (solo `valid_id` per gli id dei
//! task), `validate`; i comandi leggono `WorkspaceStore` per sapere se il canale
//! di un workspace esiste. `board` e `automation` non dipendono da `hub`.
//!
//! Un file per responsabilita':
//! - `channel`: il canale (`general` o l'id di un workspace);
//! - `model`: i DTO (thread, messaggi, autori, eventi, riassunti);
//! - `text`: validazione di titoli, testi e id;
//! - `threads`: le operazioni pure sui thread di un canale, con i limiti;
//! - `waiting`: se un thread aspetta l'utente, calcolato dagli eventi;
//! - `files`: i file `threads/<canale>.json`;
//! - `state`: i thread in memoria (caricamento pigro), stato gestito da Tauri;
//! - `settings`: `hub_settings.json` e i suoi comandi;
//! - `commands`: i comandi dei thread e l'evento.

mod channel;
pub mod commands;
mod files;
mod model;
pub mod settings;
mod state;
mod text;
mod threads;
mod waiting;

pub use settings::{load_from_disk, new_store as new_settings_store};
pub use state::HubState;

use tauri::{AppHandle, Manager};

/// Il workspace `ws_id` e' stato rimosso: via il suo canale e il file dei suoi
/// thread. Con l'HUB spento il file non esiste e non succede nulla. Gli errori
/// sono nel log: la rimozione del workspace e' gia' avvenuta.
pub fn forget_workspace(app: &AppHandle, ws_id: &str) {
    let channel = match channel::Channel::parse(ws_id) {
        Ok(channel @ channel::Channel::Workspace(_)) => channel,
        Ok(channel::Channel::General) | Err(_) => {
            log::warn!("hub: id workspace non valido, thread non eliminati");
            return;
        }
    };
    match crate::store::data_dir(app) {
        Ok(dir) => app.state::<HubState>().forget(&files::threads_dir(&dir), &channel),
        Err(e) => log::error!("hub {ws_id}: cartella dati non disponibile, thread non eliminati: {e}"),
    }
}
