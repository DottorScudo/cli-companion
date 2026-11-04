//! Tabella delle azioni: cosa si puo' fare a un task in ciascuno stato. E'
//! l'unico posto che lo dice: le guardie delle operazioni, i messaggi di errore
//! e `allowed_actions` della vista leggono tutti `permits`.

use super::super::TaskStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Azione dell'utente su un task (comando `task_action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TaskAction {
    /// `failed` -> `queued`, in fondo alla coda.
    Retry,
    /// `failed` -> `done` a mano.
    MarkDone,
    /// `running` -> `failed`, il tab del worker si chiude.
    Stop,
    /// Toglie il task.
    Delete,
    /// Toglie il task e i task non `done` che ne dipendono.
    DeleteCascade,
    /// `queued` in testa alla coda.
    ToTop,
}

impl TaskAction {
    pub const ALL: [TaskAction; 6] = [
        TaskAction::Retry,
        TaskAction::MarkDone,
        TaskAction::Stop,
        TaskAction::Delete,
        TaskAction::DeleteCascade,
        TaskAction::ToTop,
    ];
}

/// Un'operazione che guarda lo stato del task: le azioni dell'utente piu' le
/// modifiche e il riordino, che non sono azioni di `task_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Action(TaskAction),
    /// Modifica dei campi della specifica.
    Edit,
    /// Riordino della coda (`move_task`).
    Reorder,
}

impl Op {
    /// Cosa si fa al task, per i messaggi di errore ("... can be <verb>").
    pub(super) fn verb(self) -> &'static str {
        match self {
            Op::Action(TaskAction::Retry) => "retried",
            Op::Action(TaskAction::MarkDone) => "marked done",
            Op::Action(TaskAction::Stop) => "stopped",
            Op::Action(TaskAction::Delete | TaskAction::DeleteCascade) => "deleted",
            Op::Action(TaskAction::ToTop) => "moved to the top",
            Op::Edit => "updated",
            Op::Reorder => "reordered",
        }
    }

    /// Gli stati in cui l'operazione e' permessa, per i messaggi di errore ("queued or failed").
    pub(super) fn allowed(self) -> String {
        let names: Vec<&str> =
            TaskStatus::ALL.into_iter().filter(|s| permits(*s, self)).map(TaskStatus::as_str).collect();
        names.join(" or ")
    }
}

/// La tabella: `op` e' permessa su un task in `status`?
///
/// - `queued`: in testa, elimina (anche a cascata), modifica, riordina;
/// - `running`: ferma;
/// - `failed`: riprova, segna come fatto, elimina (anche a cascata), modifica;
/// - `done`: niente.
pub fn permits(status: TaskStatus, op: Op) -> bool {
    use TaskAction::{Delete, DeleteCascade, MarkDone, Retry, Stop, ToTop};
    matches!(
        (status, op),
        (TaskStatus::Queued, Op::Action(ToTop | Delete | DeleteCascade) | Op::Edit | Op::Reorder)
            | (TaskStatus::Running, Op::Action(Stop))
            | (TaskStatus::Failed, Op::Action(Retry | MarkDone | Delete | DeleteCascade) | Op::Edit)
    )
}

#[cfg(test)]
mod tests;
