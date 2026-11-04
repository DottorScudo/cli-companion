//! Il collegamento tra la board e i thread: riceve ogni `BoardView`
//! pubblicata, ne ricava l'istantanea e la passa a un thread dedicato, che fa
//! il diff con la precedente, instrada gli eventi e salva.
//!
//! Perche' un thread dedicato: `automation` chiama `board_published` tenendo
//! il lock delle pubblicazioni (cosi' le viste arrivano nell'ordine delle
//! revisioni); qui si fa solo l'istantanea e un invio sul canale, mentre il
//! lavoro su disco dell'HUB avviene dopo, fuori da ogni lock della board. Il
//! canale e' FIFO: il thread vede le viste nello stesso ordine. Non e' limitato,
//! ma i messaggi sono istantanee compatte e arrivano al ritmo delle modifiche
//! della board.
//!
//! Le istantanee stanno solo nella memoria del thread. All'avvio dell'app la
//! prima vista pubblicata di un workspace fa da base, senza eventi (il primo
//! giro dell'automazione ripubblica tutte le board). All'accensione dell'HUB
//! non si pubblica nulla: `baseline` chiede le viste correnti alla sorgente
//! registrata (`set_source`, da `lib.rs`: `hub` non dipende da `automation`) e
//! le fissa come base, cosi' gia' la prima modifica successiva produce i suoi
//! eventi. Spegnere l'HUB le butta. Il thread parte al primo invio con l'HUB
//! acceso; se muore, il prossimo invio ne avvia un altro (che riparte da zero).

use super::diff::{diff, BoardEvent};
use super::route::route;
use super::snapshot::BoardSnapshot;
use crate::board::BoardView;
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, threads_dir_of};
use crate::hub::settings::is_enabled;
use crate::hub::state::HubState;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::sync::{Mutex, OnceLock, PoisonError};
use tauri::{AppHandle, Manager};

const WORKER_NAME: &str = "hub-board-events";

/// Le viste correnti delle board di tutti i workspace, senza pubblicarle.
pub type BoardSource = fn(&AppHandle) -> Vec<BoardView>;

enum FeedMsg {
    Published(BoardSnapshot),
    /// La vista corrente di una board all'accensione: solo base, mai eventi.
    Baseline(BoardSnapshot),
    /// Il workspace e' stato rimosso.
    Forget(String),
    /// L'HUB e' stato spento.
    Reset,
}

/// Stato Tauri: il lato d'invio verso il thread, se e' partito, e la sorgente
/// delle viste per la base all'accensione.
#[derive(Default)]
pub struct HubFeed {
    tx: Mutex<Option<Sender<FeedMsg>>>,
    source: OnceLock<BoardSource>,
}

impl HubFeed {
    /// Registra la sorgente delle viste (una volta, all'avvio).
    pub fn set_source(&self, source: BoardSource) {
        if self.source.set(source).is_err() {
            log::warn!("hub: sorgente delle board gia' registrata, tengo la prima");
        }
    }

    /// Una vista appena pubblicata. Con l'HUB spento non fa nulla.
    pub fn published(&self, app: &AppHandle, view: &BoardView) {
        if is_enabled(app) {
            self.send(app, FeedMsg::Published(BoardSnapshot::of(view)));
        }
    }

    /// L'HUB e' appena stato acceso: le viste correnti di tutte le board
    /// diventano la base, senza messaggi ne' scritture. Da chiamare fuori dal
    /// main thread (legge il registro dell'automazione).
    pub fn baseline(&self, app: &AppHandle) {
        let Some(source) = self.source.get() else {
            log::warn!("hub: nessuna sorgente delle board, la base arrivera' dalla prossima pubblicazione");
            return;
        };
        for view in source(app) {
            self.send(app, FeedMsg::Baseline(BoardSnapshot::of(&view)));
        }
    }

    /// L'HUB e' stato spento: via le istantanee.
    pub fn reset(&self) {
        self.send_if_running(FeedMsg::Reset);
    }

    /// Il workspace e' stato rimosso: via la sua istantanea.
    pub fn forget(&self, ws_id: &str) {
        self.send_if_running(FeedMsg::Forget(ws_id.to_string()));
    }

    fn send(&self, app: &AppHandle, msg: FeedMsg) {
        let mut tx = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        let msg = match tx.as_ref() {
            Some(sender) => match sender.send(msg) {
                Ok(()) => return,
                Err(SendError(msg)) => {
                    log::warn!("hub: il thread degli eventi della board si e' fermato, lo riavvio");
                    msg
                }
            },
            None => msg,
        };
        let (sender, receiver) = mpsc::channel();
        let worker_app = app.clone();
        let spawned =
            std::thread::Builder::new().name(WORKER_NAME.to_string()).spawn(move || run(&worker_app, receiver));
        if let Err(e) = spawned {
            log::error!("hub: thread degli eventi della board non avviato: {e}");
            return;
        }
        // Il ricevitore e' vivo: l'invio non puo' fallire.
        let _ = sender.send(msg);
        *tx = Some(sender);
    }

    fn send_if_running(&self, msg: FeedMsg) {
        let tx = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(sender) = tx.as_ref() {
            // Un thread fermo non ha istantanee da buttare.
            let _ = sender.send(msg);
        }
    }
}

fn run(app: &AppHandle, receiver: Receiver<FeedMsg>) {
    let mut baselines = Baselines::default();
    for msg in receiver {
        match msg {
            FeedMsg::Published(next) if is_enabled(app) => {
                let (board, events) = baselines.advance(next);
                if !events.is_empty() {
                    if let Err(e) = apply(app, board, &events) {
                        log::error!("hub {}: eventi della board non riportati: {e}", board.workspace_id);
                    }
                }
            }
            FeedMsg::Baseline(base) if is_enabled(app) => baselines.seed(base),
            // Spento nel frattempo: il `Reset` e' in arrivo, si butta gia' ora.
            FeedMsg::Published(_) | FeedMsg::Baseline(_) | FeedMsg::Reset => baselines.clear(),
            FeedMsg::Forget(ws_id) => baselines.forget(&ws_id),
        }
    }
}

/// Scrive gli eventi nei thread del canale del workspace ed emette
/// `hub-update` per ogni thread toccato. Nessun salvataggio se nessun thread
/// cambia (per esempio un task eliminato che non aveva un thread).
fn apply(app: &AppHandle, board: &BoardSnapshot, events: &[BoardEvent]) -> Result<(), String> {
    let channel = match Channel::parse(&board.workspace_id)? {
        channel @ Channel::Workspace(_) => channel,
        Channel::General => return Err("una board non puo' scrivere nel Generale".to_string()),
    };
    let dir = threads_dir_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let touched = state.update_when(
        &dir,
        &channel,
        |list| {
            // Sotto il lock dell'HUB, come i comandi: niente file per un
            // workspace appena rimosso.
            ensure_channel(app, &channel)?;
            Ok(route(list, &channel, board, events, now, &mut |prefix| state.next_id(prefix, now)))
        },
        |touched| !touched.is_empty(),
    )?;
    for summary in &touched {
        emit_update(app, summary);
    }
    Ok(())
}

/// L'ultima istantanea di ogni workspace.
#[derive(Default)]
struct Baselines {
    boards: HashMap<String, BoardSnapshot>,
}

impl Baselines {
    /// Tiene `next` come nuova base e ritorna gli eventi rispetto alla
    /// precedente; nessuno se e' la prima vista del workspace. Una vista piu'
    /// vecchia della base (la base fissata all'accensione legge il registro
    /// fuori dal lock delle pubblicazioni) viene scartata: il diff all'indietro
    /// riporterebbe eventi gia' riportati o mai avvenuti.
    fn advance(&mut self, next: BoardSnapshot) -> (&BoardSnapshot, Vec<BoardEvent>) {
        match self.boards.entry(next.workspace_id.clone()) {
            Entry::Vacant(slot) => (slot.insert(next), Vec::new()),
            Entry::Occupied(slot) => {
                let prev = slot.into_mut();
                if next.rev < prev.rev {
                    return (prev, Vec::new());
                }
                let events = diff(prev, &next);
                *prev = next;
                (prev, events)
            }
        }
    }

    /// Fissa `base` come base del workspace, senza eventi, se non ne ha gia'
    /// una: una vista pubblicata nel frattempo e' gia' la base giusta.
    fn seed(&mut self, base: BoardSnapshot) {
        self.boards.entry(base.workspace_id.clone()).or_insert(base);
    }

    fn forget(&mut self, ws_id: &str) {
        self.boards.remove(ws_id);
    }

    fn clear(&mut self) {
        self.boards.clear();
    }
}

#[cfg(test)]
mod tests;
