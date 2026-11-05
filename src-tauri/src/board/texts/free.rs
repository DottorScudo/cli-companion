//! Testi dei task in modalita' libera (la cartella non e' un repository git):
//! regole, ambiente e consegna senza git. Le sceglie `EnvInfo::free`.

use super::EnvInfo;
use crate::board::TaskKind;

/// Come si chiama la consegna di un task in modalita' libera.
pub(super) const FREE_LABEL: &str = "free mode (the folder is not a git repository)";

/// Regole dei worker in modalita' libera al posto di quelle git: niente git, il report elenca i file toccati.
pub(super) const FREE_CHANGES: &str =
    "- This folder is not a git repository: work directly in it, there is nothing to commit or push. \
Never delete or revert changes you did not make.\n";
pub(super) const FREE_DONE: &str =
    "- When done, call complete_task with status \"completed\" and a report that lists the files you changed.\n";

/// Aggiunta alle istruzioni del revisore di una cartella senza git.
pub(super) const REVIEWER_FREE: &str = "\n\nThis folder is not a git repository: there is no worktree, integrated branch, commit or pull request to read, so ignore the git instructions above. \
Review the files directly in the folder.";

/// `environment` di un task in modalita' libera: la cartella senza git, niente
/// worktree, branch, base ne' setup.
pub(super) fn free_environment(e: &EnvInfo) -> String {
    let mut s = String::from("Environment of this task\n");
    s.push_str(&format!("- Workspace root: {}\n", e.root));
    if let Some(r) = &e.task.repo {
        s.push_str(&format!("- Repository: {r} (a folder of the workspace root)\n"));
    }
    s.push_str(&format!("- Folder: {} (not a git repository)\n", e.workdir));
    s.push_str(&format!("- Delivery mode: {FREE_LABEL}\n"));
    if e.task.kind == TaskKind::Review {
        s.push_str("- No git: you review the files directly in this folder: it is not a git repository, so there is no worktree or integrated branch.\n");
    } else {
        s.push_str("- No git: work directly in this folder; there is nothing to commit, push or merge.\n");
    }
    s
}

/// Consegna di un task in modalita' libera: i file restano nella cartella e il
/// report di `complete_task` dice cosa e' cambiato.
pub(super) fn free_delivery_steps(e: &EnvInfo) -> String {
    format!(
        "Delivery: {}\n1. Leave your changes in the folder: there is nothing to commit, push or merge.\n\
         Then call complete_task with status \"completed\" and a summary that lists the files you changed (created, modified, deleted).",
        e.mode_label()
    )
}
