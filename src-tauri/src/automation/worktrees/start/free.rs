//! Avvio in modalita' libera: la cartella del task non e' un repository git.
//! Il worker lavora sul posto, senza worktree, branch, setup ne' base; la
//! consegna e' lasciare i file modificati e il report di `complete_task`.

use super::Placement;
use crate::automation::planner::root;
use crate::automation::worktrees::StartJob;
use crate::automation::worktrees::{e2e_needs_git, folder_is_git};
use crate::automation::*;

/// Il task parte in modalita' libera? Un retry con il worktree e' sempre un
/// task git (niente rilevamento). Una cartella che non esiste e' un errore.
pub(super) fn is_free<H: Host>(host: &H, job: &StartJob, dir: &str) -> Result<bool, String> {
    let (ws, task) = (&job.ws, &job.task);
    if task.worktree.is_some() {
        return Ok(false);
    }
    if let Some(r) = &task.repo {
        if !host.dir_exists(ws, dir) {
            return Err(format!("the repository folder '{r}' ({dir}) does not exist in the workspace"));
        }
    }
    Ok(!folder_is_git(host, ws, dir)?)
}

/// Dove lavora un task in modalita' libera: la cartella stessa. Un piano gira
/// sul posto in sola lettura (vedi `planner::root`); gli E2E richiedono git.
pub(super) fn place<H: Host>(host: &H, job: &StartJob) -> Result<Placement, String> {
    let (ws, task) = (&job.ws, &job.task);
    if task.e2e {
        return Err(e2e_needs_git(ws, &task.id, task.repo.as_deref()));
    }
    let root_plan = if task.kind.isolated() { Some(root::start(host, ws, task)?) } else { None };
    Ok(Placement { root_plan, free: true, ..Placement::default() })
}
