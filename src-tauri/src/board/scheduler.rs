//! Scheduler: sceglie i task da avviare. Funzione pura, il contesto
//! (consegna, fascia oraria) lo fornisce chi chiama.

use super::{repo_key, Board, TaskStatus};
use crate::workspaces::Delivery;
use std::collections::HashSet;

/// Contesto dello scheduler (lo fornisce chi chiama: il nucleo non conosce
/// i workspace).
#[derive(Debug, Clone, Copy)]
pub struct SchedCtx {
    pub delivery: Delivery,
    /// Falso quando il workspace e' archiviato o usa le fasce ed e' fuori fascia.
    pub gate_open: bool,
}

/// Id dei task da avviare ora, in ordine di coda. Funzione pura.
///
/// In modalita' parallela (worktree) fino a `max_parallel` running nella
/// board; altrimenti un solo running per repository, comunque entro
/// `max_parallel`.
pub fn schedule(board: &Board, ctx: &SchedCtx) -> Vec<String> {
    if board.paused || !ctx.gate_open {
        return Vec::new();
    }
    let mut busy: HashSet<String> = HashSet::new();
    let mut running = 0usize;
    for t in board.tasks.iter().filter(|t| t.status() == TaskStatus::Running) {
        running += 1;
        busy.insert(repo_key(t.repo.as_deref()));
    }
    let parallel = ctx.delivery.worktrees();
    let mut chosen = Vec::new();
    for t in board.tasks.iter().filter(|t| t.status() == TaskStatus::Queued && board.deps_done(t)) {
        if running + chosen.len() >= board.max_parallel as usize {
            break;
        }
        if !parallel && !busy.insert(repo_key(t.repo.as_deref())) {
            continue;
        }
        chosen.push(t.id.clone());
    }
    chosen
}
