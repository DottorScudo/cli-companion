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
//! - `send`: l'invio di un messaggio dell'utente (app e canali di contatto);
//! - `ingress`: la consegna dei messaggi di mittenti esterni (connettori);
//! - `lifecycle`: gli ascoltatori di acceso/spento e della rimozione di un workspace;
//! - `attachments`: gli allegati dei messaggi dell'utente (staging, file, pulizia, comandi);
//! - `commands`: i comandi dei thread e l'evento;
//! - `thread_state`: chiudere e riaprire un thread a mano;
//! - `thread_delete`: eliminare un thread a mano (evento `hub-thread-removed`);
//! - `contact`: canali e thread aperti, e la creazione di un thread, per i canali di contatto;
//! - `events`: gli eventi della board nei thread (diff, instradamento, `HubFeed`);
//! - `engine`: il motore dei segretari (menzioni, sessioni, tool calling);
//! - `notify`: le notifiche di sistema dei messaggi dei thread (evento `hub-notify`)
//!   e i `NoticeSink` che le ricevono oltre al frontend.

pub mod action_requests;
mod actions;
mod api_key;
mod append;
pub mod attachments;
mod channel;
pub mod commands;
pub mod contact;
mod direct_answer;
pub mod engine;
mod events;
mod files;
pub mod ingress;
mod lifecycle;
mod model;
mod notify;
mod questions;
pub mod send;
pub mod settings;
mod state;
mod text;
pub mod thread_delete;
pub mod thread_state;
mod threads;
mod waiting;

pub use actions::{BoardActions, HubBoardActions};
pub use api_key::ApiKey;
pub use attachments::commands::purge_staging_in_background as purge_attachment_staging;
pub use channel::Channel;
pub use commands::thread_exists;
pub use engine::SecretaryEngine;
pub use events::{BoardSource, HubFeed};
pub use lifecycle::{HubLifecycle, HubLifecycleListeners};
pub use model::Message;
pub use model::{Author, MessageOrigin};
pub use notify::{validate_thread_ref, ContactNotice, HubNotifier, NoticeCategory, NoticeSink};
#[cfg(test)]
pub use notify::{HubNotice, HubNoticeSound};
pub use settings::{load_from_disk, new_store as new_settings_store};
pub use state::HubState;
pub use text::{valid_ref_id, BOT_NAME_MAX};

use crate::board::BoardView;
use std::sync::Arc;
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

/// Registra un destinatario delle notifiche raggruppate dell'HUB oltre al
/// frontend (un connettore che le gira a un canale di contatto). Lo registra
/// chi lo possiede, una volta; vale finche' l'app gira.
pub fn set_notice_sink(app: &AppHandle, sink: Arc<dyn NoticeSink>) {
    app.state::<HubNotifier>().add_sink(sink);
}

/// Registra un ascoltatore del ciclo di vita dell'HUB (acceso o spento, workspace
/// rimosso), per esempio il runtime dei connettori che ferma i suoi supervisori.
/// Lo registra chi lo possiede, una volta; vale finche' l'app gira.
pub fn set_lifecycle_listener(app: &AppHandle, listener: Arc<dyn HubLifecycle>) {
    app.state::<HubLifecycleListeners>().add(listener);
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
    // Gli ascoltatori (le regole dei connettori) si avvisano sempre, anche con
    // l'HUB spento e prima dei ritorni anticipati.
    app.state::<HubLifecycleListeners>().workspace_removed(app, ws_id);
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
        Ok(dir) => app.state::<HubState>().forget(&files::HubDirs::in_data_dir(&dir), &channel),
        Err(e) => log::error!("hub {ws_id}: cartella dati non disponibile, thread non eliminati: {e}"),
    }
}
