//! Le azioni dell'HUB sulla board: creare un task di pianificazione e decidere
//! una proposta. `hub` non dipende da `automation`: `lib.rs` registra le
//! funzioni reali (`set_board_actions`), come fa con la sorgente delle viste
//! (`BoardSource`). Le chiamano i tool dei segretari (`engine`), sempre fuori
//! dal main thread: creare un piano fa git.

use std::sync::OnceLock;
use tauri::AppHandle;

/// Un task di pianificazione da mettere in coda: titolo e testo completo, in
/// inglese (lo legge il pianificatore, che non vede altro).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOrder {
    pub title: String,
    pub task: String,
}

/// Crea un task `plan` in coda nel workspace e ritorna il suo id.
pub type CreatePlan = fn(&AppHandle, &str, &PlanOrder) -> Result<String, String>;
/// Accetta (`true`) o scarta la proposta `proposal_id` del workspace, con la
/// stessa validazione dei pulsanti del pannello Task.
pub type DecideProposal = fn(&AppHandle, &str, &str, bool) -> Result<(), String>;

/// Le azioni registrate da `lib.rs`.
#[derive(Clone, Copy)]
pub struct BoardActions {
    pub create_plan: CreatePlan,
    pub decide_proposal: DecideProposal,
}

/// Stato Tauri: le azioni, registrate una volta all'avvio.
#[derive(Default)]
pub struct HubBoardActions {
    actions: OnceLock<BoardActions>,
}

impl HubBoardActions {
    pub fn set(&self, actions: BoardActions) {
        if self.actions.set(actions).is_err() {
            log::warn!("hub: azioni sulla board gia' registrate, tengo le prime");
        }
    }

    /// Le azioni registrate; un errore (in inglese, per il modello) se mancano.
    pub fn get(&self) -> Result<BoardActions, String> {
        self.actions.get().copied().ok_or_else(|| {
            log::error!("hub: nessuna azione sulla board registrata");
            "The board cannot be changed right now.".to_string()
        })
    }
}
