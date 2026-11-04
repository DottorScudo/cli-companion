//! Git dell'automazione: tipi e helper condivisi (percorsi, comandi git,
//! verifiche dei repository) e accesso ai file della board. Generico
//! sull'`Host`, quindi si prova con un host finto; gira sempre fuori dal lock
//! del registro, su un thread. L'avvio di un task e' in `start`, la pulizia a
//! consegna avvenuta in `cleanup`, il lato reale di git (esecuzione,
//! rilevamento dei repository) in `real` e il riallineamento delle worktree
//! orfane in `reconcile`.

use super::*;
use crate::board::{repo_key, Task, TaskSpec, WorktreeInfo};
use crate::platform::TargetKind;
use crate::workspaces::RepoConfig;
use std::path::PathBuf;

mod cleanup;
mod real;
mod reconcile;
mod start;
#[cfg(test)]
mod tests;

pub(super) use cleanup::run_cleanup;
#[cfg(test)]
pub(super) use real::parse_repo_list;
pub(super) use real::{append_exclude_line, detect_repos, run_git};
#[cfg(test)]
pub(super) use reconcile::ReconcileReport;
pub(super) use reconcile::Reconciler;
pub(super) use start::run_start_job;

/// Esito di un comando git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOut {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Avvio di un task scelto dallo scheduler: tutto cio' che serve fuori dal
/// lock (git + spawn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartJob {
    pub ws: Workspace,
    pub task: Task,
    /// `attempts` al momento della scelta: scarta avvii superati da uno
    /// stop/retry.
    pub attempt: u32,
}

/// Worktree da rimuovere (task concluso o eliminato, o orfana).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupTarget {
    pub ws_id: String,
    pub task_id: String,
    pub repo: Option<String>,
    pub wt: WorktreeInfo,
    /// Il workspace non e' piu' nello store (rimosso con la sua board): la
    /// pulizia usa questa copia invece di cercarlo per id.
    pub workspace: Option<Workspace>,
    /// Il branch si cancella solo se e' unito (`-d`) anche in modalita' `pr`:
    /// per le worktree orfane la consegna non e' confermata da nessuno.
    pub keep_unmerged: bool,
}

impl CleanupTarget {
    /// Pulizia del worktree di `task` (se ne ha uno) dopo una consegna
    /// confermata o un'eliminazione voluta dall'utente.
    pub fn for_task(ws_id: &str, task: Task) -> Option<Self> {
        let wt = task.worktree?;
        Some(Self {
            ws_id: ws_id.to_string(),
            task_id: task.id,
            repo: task.repo,
            wt,
            workspace: None,
            keep_unmerged: false,
        })
    }

    /// Lo stesso, per un workspace che non e' piu' nello store.
    pub fn with_workspace(self, ws: &Workspace) -> Self {
        Self { workspace: Some(ws.clone()), ..self }
    }
}

/// Cartella, nel repository, che contiene i worktree dei task.
const WORKTREES_DIR: &str = ".worktrees";
/// Prefisso dei branch dei task: `task/<id>`.
const TASK_BRANCH_PREFIX: &str = "task/";
/// Tetto di un `git fetch` (il resto dei comandi git ha `GIT_TIMEOUT`).
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Tetto di un comando git (o wsl.exe): oltre, il processo si uccide e
/// l'avvio fallisce invece di bloccare per sempre il thread.
pub(super) const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Cache dei fetch: (workspace|cartella|base) -> (istante, avviso).
pub(super) type FetchCache = HashMap<String, (std::time::Instant, Option<String>)>;

// ---------------------------------------------------------------------------
// Git e percorsi
// ---------------------------------------------------------------------------

/// Branch del task `id`.
fn task_branch(id: &str) -> String {
    format!("{TASK_BRANCH_PREFIX}{id}")
}

/// Esegue git e ritorna lo stdout (trim); fallimento = errore leggibile.
pub(super) fn git<H: Host>(host: &H, ws: &Workspace, dir: &str, args: &[&str]) -> Result<String, String> {
    let out = host.git(ws, dir, args).map_err(|e| format!("git is not available: {e}"))?;
    if out.ok {
        Ok(out.stdout.trim().to_string())
    } else {
        let why = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
        Err(format!("git {} failed: {}", args.join(" "), clip_chars(why, 500)))
    }
}

/// Unisce un path relativo (con `/`) al path della root nell'ambiente del
/// workspace: `\` su Windows, `/` su WSL.
pub(super) fn join_path(kind: TargetKind, root: &str, rel: &str) -> String {
    let sep = if kind == TargetKind::Wsl { '/' } else { '\\' };
    let root = root.trim_end_matches(['/', '\\']);
    let rel: String = rel.chars().map(|c| if c == '/' { sep } else { c }).collect();
    format!("{root}{sep}{rel}")
}

fn is_abs(kind: TargetKind, p: &str) -> bool {
    if kind == TargetKind::Wsl {
        return p.starts_with('/');
    }
    let b = p.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    drive || p.starts_with("\\\\") || p.starts_with("//")
}

/// Cartella del repository di un task nell'ambiente del workspace: la root
/// se `repo` e' `None`, altrimenti `repo` sotto la root.
pub(super) fn repo_dir(ws: &Workspace, repo: Option<&str>) -> String {
    match repo {
        Some(r) => join_path(ws.kind, &ws.path, r),
        None => ws.path.clone(),
    }
}

fn repo_cfg<'a>(ws: &'a Workspace, repo: Option<&str>) -> Option<&'a RepoConfig> {
    ws.repos.get(&repo_key(repo))
}

/// Un task `e2e` richiede che il suo repository abbia gli E2E configurati.
pub(super) fn check_e2e_configured(ws: &Workspace, specs: &[TaskSpec]) -> Result<(), String> {
    match specs.iter().find(|s| s.e2e && !repo_cfg(ws, s.repo.as_deref()).is_some_and(|c| c.e2e.is_some())) {
        Some(s) => Err(format!(
            "task '{}': e2e requires an E2E command configured for repository '{}'",
            s.id,
            repo_key(s.repo.as_deref())
        )),
        None => Ok(()),
    }
}

/// I repository esistono e, se la modalita' di consegna usa i branch, sono
/// repo git. Gira fuori dal lock (git).
pub(super) fn ensure_repos<'a, H: Host>(
    host: &H,
    ws: &Workspace,
    repos: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<(), String> {
    let mut seen: Vec<Option<&str>> = Vec::new();
    for repo in repos {
        if seen.contains(&repo) {
            continue;
        }
        seen.push(repo);
        let dir = repo_dir(ws, repo);
        if let Some(r) = repo {
            if !host.dir_exists(ws, &dir) {
                return Err(format!("the repository folder '{r}' ({dir}) does not exist in the workspace"));
            }
        }
        if ws.delivery.uses_branches() {
            let inside = host
                .git(ws, &dir, &["rev-parse", "--is-inside-work-tree"])
                .map_err(|e| format!("git is not available: {e}"))?;
            if !(inside.ok && inside.stdout.trim() == "true") {
                return Err(format!(
                    "{dir} is not a git repository: the delivery mode of this workspace works on branches and needs one. \
                     Fix the repo of the task or ask the user to run git init there"
                ));
            }
        }
    }
    Ok(())
}

/// `git diff --stat <base>...HEAD` piu' i file non committati, troncato.
/// Gira nel worktree del task, che sta nel repository del task.
pub(super) fn compute_diffstat<H: Host>(host: &H, ws: &Workspace, wt: &WorktreeInfo) -> Option<String> {
    let range = format!("{}...HEAD", wt.base);
    let stat = git(host, ws, &wt.path, &["diff", "--stat", &range]).unwrap_or_default();
    let dirty = git(host, ws, &wt.path, &["status", "--porcelain"])
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    let mut out = if stat.is_empty() { "no committed changes".to_string() } else { stat };
    if dirty > 0 {
        out.push_str(&format!("\n{dirty} file(s) with uncommitted changes"));
    }
    Some(clip_chars(&out, board::DIFFSTAT_MAX))
}

pub(super) fn boards_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("boards")).map_err(|e| e.to_string())
}

/// File della board; l'id del workspace non deve poter uscire dalla cartella.
pub(super) fn board_file(dir: &std::path::Path, ws_id: &str) -> Result<PathBuf, String> {
    if ws_id.is_empty() || !ws_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("id workspace non valido".to_string());
    }
    Ok(dir.join(format!("{ws_id}.json")))
}
