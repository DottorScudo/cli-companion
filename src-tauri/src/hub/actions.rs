//! Le azioni dell'HUB sulla board: creare un task di pianificazione, decidere
//! una proposta, rispondere alla domanda di un worker, agire su un task
//! (ritentarlo, riordinarlo, modificarlo) e cambiare pausa e parallelismo della board.
//! `hub` non dipende da
//! `automation`: `lib.rs` registra le funzioni reali (`set_board_actions`),
//! come fa con la sorgente delle viste (`BoardSource`). Le chiamano i tool dei
//! segretari (`engine`) e la risposta diretta (`direct_answer`), sempre fuori
//! dal main thread: creare un piano fa git, rispondere scrive nel terminale.

use super::questions::{QuestionBoard, QuestionRef};
use crate::board::{BoardView, TaskAction};
use serde_json::Value;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

/// Nessuna azione registrata, per l'utente.
const ACTIONS_MISSING: &str = "le azioni sulla board non sono disponibili.";

/// Un task di pianificazione da mettere in coda: titolo e testo completo, in
/// inglese (lo legge il pianificatore, che non vede altro), e il Bot che lo
/// esegue (agente, modello, effort e skill vengono da lui).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOrder {
    pub title: String,
    pub task: String,
    /// Id di un Bot-agente.
    pub bot: String,
}

/// Crea un task `plan` in coda nel workspace e ritorna il suo id.
pub type CreatePlan = fn(&AppHandle, &str, &PlanOrder) -> Result<String, String>;
/// Accetta (`true`) o scarta la proposta `proposal_id` del workspace, con la
/// stessa validazione dei pulsanti del pannello Task.
pub type DecideProposal = fn(&AppHandle, &str, &str, bool) -> Result<(), String>;

/// Consegna `text` alla domanda `question_id` del task `task_id` del
/// workspace: la segna risposta e la scrive nel terminale del worker, se e'
/// fermo in attesa. Errori in italiano (li legge l'utente).
pub type AnswerQuestion = fn(&AppHandle, &str, &str, &str, &str) -> Result<(), String>;

/// Esegue l'azione sul task (terzo argomento) del workspace, con la stessa
/// validazione del comando `task_action` del pannello Task; ritorna la board
/// aggiornata.
pub type ActOnTask = fn(&AppHandle, &str, &str, TaskAction) -> Result<BoardView, String>;
/// Cambia i campi del task (terzo argomento) con `patch` (titolo, specifica,
/// dipendenze, modello), come il comando `task_update`; ritorna la board
/// aggiornata.
pub type UpdateTask = fn(&AppHandle, &str, &str, &Value) -> Result<BoardView, String>;
/// Verifica `patch` come `UpdateTask` (stato, grafo, modello dell'agente) senza
/// cambiare nulla: l'errore che darebbe la modifica, o `Ok`.
pub type CheckTaskUpdate = fn(&AppHandle, &str, &str, &Value) -> Result<(), String>;
/// Sposta il task `queued` (terzo argomento) prima di un altro task `queued`
/// (quarto), come il comando `task_move`; ritorna la board aggiornata.
pub type MoveTask = fn(&AppHandle, &str, &str, &str) -> Result<BoardView, String>;
/// Imposta `max_parallel` e/o la pausa della board del workspace, con gli
/// stessi limiti del comando `board_set`; ritorna la board aggiornata.
pub type SetBoard = fn(&AppHandle, &str, Option<u8>, Option<bool>) -> Result<BoardView, String>;

/// Le azioni registrate da `lib.rs`.
#[derive(Clone, Copy)]
pub struct BoardActions {
    pub create_plan: CreatePlan,
    pub decide_proposal: DecideProposal,
    pub answer_question: AnswerQuestion,
    pub act_on_task: ActOnTask,
    pub update_task: UpdateTask,
    pub check_task_update: CheckTaskUpdate,
    pub move_task: MoveTask,
    pub set_board: SetBoard,
}

/// Le risposte alle domande dei worker sull'app: la risposta diretta del thread
/// e `relay_answer` dei segretari.
pub struct AppAnswers<'a> {
    pub app: &'a AppHandle,
}

impl QuestionBoard for AppAnswers<'_> {
    fn answer(&self, question: &QuestionRef, text: &str) -> Result<(), String> {
        let actions = self.app.state::<HubBoardActions>().get().map_err(|_| ACTIONS_MISSING.to_string())?;
        (actions.answer_question)(self.app, &question.workspace, &question.task_id, &question.question_id, text)
    }
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
