//! Agents HUB: modalita' opzionale, spenta di default, con un canale per
//! workspace piu' il Generale e, dentro ogni canale, thread di messaggi.
//! Porta l'impostazione globale, il modello dati, la persistenza, i comandi
//! Tauri, l'evento `hub-update`, gli eventi della board nei thread e la
//! configurazione dei segretari (provider, lingua, sessioni, elenco, API key nel
//! portachiavi) e il motore che li fa parlare. Documentazione: docs/HUB.MD.
//!
//! Dipendenze: `store` (persistenza), `secrets` (portachiavi), `bots` (validazione
//! delle immagini), `board` (`valid_id` per gli id dei task e
//! `BoardView` per gli eventi della board), `validate`; i comandi leggono
//! `WorkspaceStore` per sapere se il canale di un workspace esiste. `board` non
//! dipende da `hub`; `automation` lo raggiunge solo dall'host reale
//! (`TauriHost::board_published` -> `board_published`). Per la base delle board
//! all'accensione `hub` riceve da `lib.rs` una sorgente delle viste
//! (`set_board_source`, con `automation::board_views`), senza dipendere da
//! `automation`. Il motore dei segretari (`engine`) chiama il modello con
//! `async-openai` e legge le board dalla stessa sorgente; per agire sulla board
//! (task di pianificazione, decisione di una proposta, risposta alla domanda di
//! un worker) usa le azioni che `lib.rs` registra allo stesso modo
//! (`set_board_actions`).
//!
//! Un file per responsabilita':
//! - `actions`: le azioni sulla board registrate da `lib.rs`;
//! - `channel`: il canale (`general` o l'id di un workspace);
//! - `model`: i DTO (thread, messaggi, autori, eventi, riassunti);
//! - `text`: validazione e pulizia di titoli, testi e id;
//! - `threads`: le operazioni pure sui thread di un canale, con i limiti;
//! - `waiting`: se un thread aspetta l'utente, calcolato dagli eventi;
//! - `questions`: le domande dei worker che aspettano una risposta in un thread;
//! - `direct_answer`: "Rispondi" su una domanda consegna il testo al worker,
//!   senza il modello;
//! - `files`: i file `threads/<canale>.json`;
//! - `state`: i thread in memoria (caricamento pigro), stato gestito da Tauri;
//! - `settings`: `hub_settings.json` (interruttore, provider, lingua, sessioni,
//!   segretari) e i suoi comandi;
//! - `api_key`: la API key del provider nel portachiavi del sistema;
//! - `append`: l'aggiunta di un messaggio di testo con salvataggio ed evento;
//! - `commands`: i comandi dei thread e l'evento;
//! - `events`: gli eventi della board nei thread (diff, instradamento, `HubFeed`);
//! - `engine`: il motore dei segretari (menzioni, sessioni, tool calling).

mod actions;
mod api_key;
mod append;
mod channel;
pub mod commands;
mod direct_answer;
pub mod engine;
mod events;
mod files;
mod model;
mod questions;
pub mod settings;
mod state;
mod text;
mod threads;
mod waiting;

pub use actions::{BoardActions, HubBoardActions};
pub use api_key::ApiKey;
pub use engine::SecretaryEngine;
pub use events::{BoardSource, HubFeed};
pub use settings::{load_from_disk, new_store as new_settings_store};
pub use state::HubState;

use crate::board::BoardView;
use tauri::{AppHandle, Manager};

/// Registra la sorgente delle viste correnti delle board, usata all'accensione
/// dell'HUB per fissarne la base. La registra `lib.rs` con
/// `automation::board_views`: cosi' `hub` non dipende da `automation`.
pub fn set_board_source(app: &AppHandle, source: BoardSource) {
    app.state::<HubFeed>().set_source(source);
}

/// Registra le azioni dell'HUB sulla board (task di pianificazione, decisione
/// di una proposta), usate dai tool dei segretari. Le registra `lib.rs` con le
/// funzioni di `automation`, come la sorgente delle viste.
pub fn set_board_actions(app: &AppHandle, actions: BoardActions) {
    app.state::<HubBoardActions>().set(actions);
}

/// Una vista della board e' stata appena pubblicata (revisione nuova). Con
/// l'HUB spento non fa nulla; acceso, accoda l'istantanea per gli eventi nei
/// thread. Veloce: il chiamante tiene il lock delle pubblicazioni.
pub fn board_published(app: &AppHandle, view: &BoardView) {
    app.state::<HubFeed>().published(app, view);
}

/// Il workspace `ws_id` e' stato rimosso: via il suo canale, il file dei suoi
/// thread e l'istantanea della sua board. Con l'HUB spento il file non esiste e
/// non succede nulla. Gli errori sono nel log: la rimozione del workspace e'
/// gia' avvenuta.
pub fn forget_workspace(app: &AppHandle, ws_id: &str) {
    app.state::<HubFeed>().forget(ws_id);
    let channel = match channel::Channel::parse(ws_id) {
        Ok(channel @ channel::Channel::Workspace(_)) => channel,
        Ok(channel::Channel::General) | Err(_) => {
            log::warn!("hub: id workspace non valido, thread non eliminati");
            return;
        }
    };
    app.state::<SecretaryEngine>().forget_channel(app, &channel);
    match crate::store::data_dir(app) {
        Ok(dir) => app.state::<HubState>().forget(&files::threads_dir(&dir), &channel),
        Err(e) => log::error!("hub {ws_id}: cartella dati non disponibile, thread non eliminati: {e}"),
    }
}
