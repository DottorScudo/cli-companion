//! Stato di un worker come lo vedono board e frontend, e la tabella di
//! transizione pura che gli hook dell'agente applicano agli stati non finali.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stato di un worker sul filo (evento `automation-update`): l'unione degli
/// stati attivi (`WorkerActivity`) e di quelli finali (`WorkerEnd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum WorkerStatus {
    /// Spawnato, task non ancora letto.
    Starting,
    Running,
    /// Turno chiuso in attesa di lavoro in background (Claude Code riprende
    /// da solo): non e' un abbandono.
    Background,
    /// Permesso in attesa nel terminale.
    NeedsInput,
    /// L'agente ha finito il turno senza `complete_task`.
    Idle,
    /// Ha fatto una domanda con `ask_user` e aspetta la risposta dell'utente: non
    /// e' un abbandono ne' un permesso.
    AwaitingAnswer,
    Completed,
    Failed,
    Closed,
}

impl WorkerStatus {
    /// L'attivita' del worker; `None` per gli stati finali (`WorkerEnd`), dopo i
    /// quali non ci sono piu' transizioni dagli hook.
    pub fn activity(self) -> Option<WorkerActivity> {
        match self {
            WorkerStatus::Starting => Some(WorkerActivity::Starting),
            WorkerStatus::Running => Some(WorkerActivity::Running),
            WorkerStatus::Background => Some(WorkerActivity::Background),
            WorkerStatus::NeedsInput => Some(WorkerActivity::NeedsInput),
            WorkerStatus::Idle => Some(WorkerActivity::Idle),
            WorkerStatus::AwaitingAnswer => Some(WorkerActivity::AwaitingAnswer),
            WorkerStatus::Completed | WorkerStatus::Failed | WorkerStatus::Closed => None,
        }
    }
}

/// Gli stati di un worker ancora attivo (non finito).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerActivity {
    /// Spawnato, task non ancora letto.
    Starting,
    Running,
    /// Turno chiuso in attesa di lavoro in background.
    Background,
    /// Permesso in attesa nel terminale.
    NeedsInput,
    /// L'agente ha finito il turno senza `complete_task`.
    Idle,
    /// Ha fatto una domanda con `ask_user` e aspetta la risposta dell'utente.
    AwaitingAnswer,
}

impl WorkerActivity {
    /// Il worker aspetta l'utente (un permesso o un lavoro da riprendere). Chi ha fatto
    /// una domanda aspetta l'utente ma non e' "attenzione": la domanda ha la sua notifica.
    pub fn needs_attention(self) -> bool {
        matches!(self, WorkerActivity::NeedsInput | WorkerActivity::Idle)
    }
}

impl From<WorkerActivity> for WorkerStatus {
    fn from(a: WorkerActivity) -> WorkerStatus {
        match a {
            WorkerActivity::Starting => WorkerStatus::Starting,
            WorkerActivity::Running => WorkerStatus::Running,
            WorkerActivity::Background => WorkerStatus::Background,
            WorkerActivity::NeedsInput => WorkerStatus::NeedsInput,
            WorkerActivity::Idle => WorkerStatus::Idle,
            WorkerActivity::AwaitingAnswer => WorkerStatus::AwaitingAnswer,
        }
    }
}

/// Come e' finito un worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerEnd {
    /// `complete_task` con esito `completed`.
    Completed,
    /// `complete_task` con esito `failed`.
    Failed,
    /// Tab chiuso o sessione finita senza `complete_task`, oppure staccato.
    Closed,
}

impl From<WorkerEnd> for WorkerStatus {
    fn from(e: WorkerEnd) -> WorkerStatus {
        match e {
            WorkerEnd::Completed => WorkerStatus::Completed,
            WorkerEnd::Failed => WorkerStatus::Failed,
            WorkerEnd::Closed => WorkerStatus::Closed,
        }
    }
}

/// Cio' che un hook o l'utente comunicano a un worker attivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerSignal {
    /// Il terminale chiede un permesso.
    PermissionPrompt,
    /// L'agente ha chiuso il turno senza `complete_task`.
    TurnEnded {
        /// Lavoro in background attivo: Claude Code riprende da solo.
        background_work: bool,
        /// Lo Stop hook era gia' stato bloccato una volta.
        stop_hook_active: bool,
        /// Il task ha una domanda in attesa di risposta (`ask_user`).
        question_pending: bool,
    },
    /// L'utente ha premuto Invio nel terminale.
    UserInput,
    /// Il worker ha chiamato `get_task`.
    TaskRead,
}

/// Esito di un segnale: il nuovo stato (se cambia) e se lo Stop hook va
/// bloccato per far proseguire il turno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerStep {
    pub to: Option<WorkerActivity>,
    pub block_stop: bool,
}

impl WorkerActivity {
    /// Tabella di transizione dei worker attivi. Funzione pura; i worker
    /// finiti (`WorkerEnd`) ignorano ogni segnale e non arrivano qui.
    pub fn on(self, signal: WorkerSignal) -> WorkerStep {
        let stay = WorkerStep { to: None, block_stop: false };
        let go = |to: WorkerActivity| WorkerStep { to: (to != self).then_some(to), block_stop: false };
        match signal {
            WorkerSignal::PermissionPrompt => go(WorkerActivity::NeedsInput),
            WorkerSignal::TurnEnded { background_work: true, .. } => go(WorkerActivity::Background),
            // Domanda all'utente: si aspetta la risposta, senza sollecitare il worker.
            WorkerSignal::TurnEnded { background_work: false, question_pending: true, .. } => {
                go(WorkerActivity::AwaitingAnswer)
            }
            // Prima volta senza report: si fa proseguire il turno. Un nuovo
            // turno dopo la risposta dell'utente non e' piu' attesa.
            WorkerSignal::TurnEnded { background_work: false, stop_hook_active: false, question_pending: false } => {
                let to = self.needs_attention().then_some(WorkerActivity::Running);
                WorkerStep { to, block_stop: true }
            }
            // Gia' sollecitato e ancora nessun report: aspetta l'utente.
            WorkerSignal::TurnEnded { background_work: false, stop_hook_active: true, question_pending: false } => {
                go(WorkerActivity::Idle)
            }
            WorkerSignal::UserInput if self.needs_attention() || self == WorkerActivity::AwaitingAnswer => {
                go(WorkerActivity::Running)
            }
            WorkerSignal::TaskRead if self == WorkerActivity::Starting => go(WorkerActivity::Running),
            WorkerSignal::UserInput | WorkerSignal::TaskRead => stay,
        }
    }
}

#[cfg(test)]
mod tests;
