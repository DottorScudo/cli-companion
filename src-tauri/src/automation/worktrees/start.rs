//! Avvio di un task: base, fetch, worktree, setup del repository, comando di
//! lancio e spawn del worker. Gira su un thread dell'host, fuori dal lock.

use super::{git, is_abs, join_path, repo_cfg, repo_dir, task_branch, FetchCache, StartJob, WORKTREES_DIR};
use crate::agents::AutomationRequest;
use crate::automation::hooks;
use crate::automation::*;
use crate::board::{repo_key, WorktreeInfo};
use crate::validate::valid_branch;
use crate::workspaces::DeliveryMode;

const EXCLUDE_LINE: &str = ".worktrees/";
/// Quanto vale un `git fetch` gia' fatto per (repo, base).
const FETCH_TTL: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Base, fetch, worktree
// ---------------------------------------------------------------------------

/// Branch base di un repository: `base_branch` del repository, poi (solo `pr`)
/// il default di `origin`, poi il branch corrente della cartella.
fn resolve_base<H: Host>(host: &H, ws: &Workspace, dir: &str, repo: Option<&str>) -> Result<String, String> {
    if let Some(b) = repo_cfg(ws, repo).and_then(|c| c.base_branch.as_deref()).filter(|b| valid_branch(b)) {
        return Ok(b.to_string());
    }
    if ws.delivery.mode == DeliveryMode::Pr {
        if let Ok(r) = git(host, ws, dir, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
            let b = r.strip_prefix("origin/").unwrap_or(&r);
            if valid_branch(b) {
                return Ok(b.to_string());
            }
        }
    }
    let cur = current_branch(host, ws, dir)?;
    if cur.is_empty() || cur == "HEAD" {
        let what = repo.map_or("the workspace".to_string(), |r| format!("the repository '{r}'"));
        return Err(format!("{what} is in detached HEAD state: check out a branch or set the repository base branch in the workspace settings"));
    }
    Ok(cur)
}

fn current_branch<H: Host>(host: &H, ws: &Workspace, dir: &str) -> Result<String, String> {
    git(host, ws, dir, &["rev-parse", "--abbrev-ref", "HEAD"])
}

fn ref_exists<H: Host>(host: &H, ws: &Workspace, dir: &str, name: &str) -> bool {
    host.git(ws, dir, &["rev-parse", "--verify", "--quiet", name]).map(|o| o.ok).unwrap_or(false)
}

/// `git fetch origin <base>` (al massimo una volta ogni `FETCH_TTL` per
/// repository e base; salta se manca `origin`) e riferimento da cui creare:
/// `origin/<base>` se esiste, altrimenti il branch locale. Un fetch fallito
/// non blocca: si prosegue con i riferimenti locali e c'e' un avviso.
fn remote_or_local<H: Host>(
    host: &H,
    ws: &Workspace,
    dir: &str,
    base: &str,
    cache: &mut FetchCache,
    warnings: &mut Vec<String>,
) -> String {
    let key = format!("{}|{dir}|{base}", ws.id);
    let warn = match cache.get(&key).filter(|(at, _)| at.elapsed() < FETCH_TTL) {
        Some((_, w)) => w.clone(),
        None => {
            let has_origin = host.git(ws, dir, &["remote", "get-url", "origin"]).map(|o| o.ok).unwrap_or(false);
            let warn = if !has_origin {
                None
            } else {
                match host.git(ws, dir, &["fetch", "origin", base]) {
                    Ok(o) if o.ok => None,
                    Ok(o) => Some(format!(
                        "git fetch origin {base} failed ({}): using the local refs",
                        clip_chars(o.stderr.trim(), 200)
                    )),
                    Err(e) => Some(format!("git fetch origin {base} failed ({e}): using the local refs")),
                }
            };
            cache.insert(key, (std::time::Instant::now(), warn.clone()));
            warn
        }
    };
    if let Some(w) = warn.filter(|w| !warnings.contains(w)) {
        warnings.push(w);
    }
    let remote = format!("origin/{base}");
    if ref_exists(host, ws, dir, &format!("refs/remotes/{remote}")) {
        remote
    } else {
        base.to_string()
    }
}

/// `.worktrees/` fuori dal git status del repository, senza toccare .gitignore.
fn add_worktrees_exclude<H: Host>(host: &H, ws: &Workspace, dir: &str, warnings: &mut Vec<String>) {
    match git(host, ws, dir, &["rev-parse", "--git-path", "info/exclude"]) {
        Ok(rel) => {
            let file = if is_abs(ws.kind, &rel) { rel } else { join_path(ws.kind, dir, &rel) };
            if let Err(e) = host.append_exclude(ws, &file, EXCLUDE_LINE) {
                warnings.push(format!(
                    "{EXCLUDE_LINE} not added to the git exclude file ({e}): it may show up in git status"
                ));
            }
        }
        Err(e) => warnings.push(format!("git exclude file not found ({e}): {EXCLUDE_LINE} may show up in git status")),
    }
}

/// Setup di un worktree appena creato (file ignorati, comandi), se il
/// repository ne ha uno. Ogni passo e' pubblicato e fa da battito del guardiano.
fn setup_worktree<H: Host>(
    st: &AutomationState,
    host: &H,
    job: &StartJob,
    dir: &str,
    wt: &WorktreeInfo,
) -> Result<Vec<String>, String> {
    let (ws, task) = (&job.ws, &job.task);
    let Some(cfg) = repo_cfg(ws, task.repo.as_deref()).filter(|c| c.has_setup()) else {
        return Ok(Vec::new());
    };
    let key = repo_key(task.repo.as_deref());
    let ctx = hooks::SetupCtx {
        ws,
        cfg,
        repo: &key,
        repo_dir: dir,
        worktree: &wt.path,
        task_id: &task.id,
        branch: &wt.branch,
        base: &wt.base,
    };
    let report = |label: &str| {
        st.run(host, |r| r.touch_preparing(&ws.id, &task.id, job.attempt, Some(label), now_millis(), true));
    };
    hooks::run_setup(host, &ctx, &report)
}

/// Crea (o riusa) il worktree del task nel suo repository, dal branch base:
/// `origin/<base>` dopo un fetch (`pr`) o il `<base>` locale (`local_merge`).
/// Ritorna anche gli avvisi (non bloccanti) per la nota del task.
fn prepare_worktree<H: Host>(
    st: &AutomationState,
    host: &H,
    job: &StartJob,
    base: &str,
) -> Result<(WorktreeInfo, Vec<String>), String> {
    let (ws, task) = (&job.ws, &job.task);
    let dir = repo_dir(ws, task.repo.as_deref());
    // Retry: il worktree gia' creato si riusa cosi' com'e'.
    if let Some(wt) = task.worktree.as_ref().filter(|wt| host.dir_exists(ws, &wt.path)) {
        return Ok((wt.clone(), Vec::new()));
    }
    let mut warnings = Vec::new();
    let base_ref = if ws.delivery.mode == DeliveryMode::Pr {
        // Un fetch alla volta: la cache evita di ripeterli per ogni task.
        let mut cache = lock(&st.fetches);
        remote_or_local(host, ws, &dir, base, &mut cache, &mut warnings)
    } else {
        base.to_string()
    };
    let path = task
        .worktree
        .as_ref()
        .map(|w| w.path.clone())
        .unwrap_or_else(|| join_path(ws.kind, &dir, &format!("{WORKTREES_DIR}/{}", task.id)));
    let mut info = WorktreeInfo { path, branch: task_branch(&task.id), base: base_ref, setup_done: true };
    // Cartella gia' presente ma non registrata (non e' nostra): niente setup.
    if host.dir_exists(ws, &info.path) {
        return Ok((info, warnings));
    }
    add_worktrees_exclude(host, ws, &dir, &mut warnings);
    let _ = git(host, ws, &dir, &["worktree", "prune"]);
    if ref_exists(host, ws, &dir, &format!("refs/heads/{}", info.branch)) {
        git(host, ws, &dir, &["worktree", "add", &info.path, &info.branch])?;
    } else {
        git(host, ws, &dir, &["worktree", "add", &info.path, "-b", &info.branch, &info.base])?;
    }
    // Appena creato: il setup e' da fare. Si registra subito, cosi' un setup
    // fallito (o un'app chiusa a meta') lascia traccia e il retry lo rifa.
    info.setup_done = false;
    st.run(host, |r| r.record_worktree(&ws.id, &task.id, job.attempt, info.clone()));
    Ok((info, warnings))
}

/// Worktree del task pronto: creato (o riusato) e con il setup del
/// repository fatto. Se c'e' qualcosa di lento da fare il task risulta "in
/// preparazione" e il guardiano dell'avvio aspetta (vedi `PREPARING_TIMEOUT_MS`).
/// Un setup fallito lascia il worktree com'e' (registrato con `setup_done`
/// falso) e fallisce l'avvio.
fn prepare_and_setup<H: Host>(
    st: &AutomationState,
    host: &H,
    job: &StartJob,
    base: &str,
) -> Result<(WorktreeInfo, Vec<String>), String> {
    let (ws, task) = (&job.ws, &job.task);
    let slow = repo_cfg(ws, task.repo.as_deref()).is_some_and(|c| c.has_setup());
    if slow {
        st.run(host, |r| r.touch_preparing(&ws.id, &task.id, job.attempt, Some("worktree"), now_millis(), true));
    }
    let end = || {
        if slow {
            st.run(host, |r| r.end_preparing(&ws.id, &task.id, job.attempt, now_millis()));
        }
    };
    let result = prepare_worktree(st, host, job, base).and_then(|(mut wt, mut warnings)| {
        if !wt.setup_done {
            let dir = repo_dir(ws, task.repo.as_deref());
            warnings.extend(setup_worktree(st, host, job, &dir, &wt)?);
            wt.setup_done = true;
            // Registrato fatto subito: se un passo successivo dell'avvio fallisce, il retry non rifa il setup.
            st.run(host, |r| r.record_worktree(&ws.id, &task.id, job.attempt, wt.clone()));
        }
        Ok((wt, warnings))
    });
    end();
    result
}

// ---------------------------------------------------------------------------
// Avvio di un task (thread separato)
// ---------------------------------------------------------------------------

struct Prepared {
    worktree: Option<WorktreeInfo>,
    warnings: Vec<String>,
    new: NewWorker,
    env: WorkerEnv,
}

/// Con `local_merge` il fast-forward avviene nel checkout del repository: deve
/// stare sul branch base.
fn require_checkout_on_base<H: Host>(host: &H, ws: &Workspace, dir: &str, base: &str) -> Result<(), String> {
    let cur = current_branch(host, ws, dir)?;
    if cur == base {
        Ok(())
    } else {
        Err(format!("local_merge needs {dir} checked out on {base} (now on {cur})"))
    }
}

/// Base del task: quella del worktree da riusare (retry), altrimenti risolta adesso.
fn task_base<H: Host>(host: &H, job: &StartJob, dir: &str) -> Result<String, String> {
    match &job.task.worktree {
        Some(wt) => Ok(wt.base.strip_prefix("origin/").unwrap_or(&wt.base).to_string()),
        None => resolve_base(host, &job.ws, dir, job.task.repo.as_deref()),
    }
}

fn prepare_start<H: Host>(st: &AutomationState, host: &H, job: &StartJob) -> Result<Prepared, String> {
    let (ws, task) = (&job.ws, &job.task);
    let delivery = ws.delivery;
    let dir = repo_dir(ws, task.repo.as_deref());
    let branch = delivery.uses_branches().then(|| task_branch(&task.id));
    let (worktree, warnings, base) = if delivery.worktrees() {
        let base = task_base(host, job, &dir)?;
        if delivery.mode == DeliveryMode::LocalMerge {
            require_checkout_on_base(host, ws, &dir, &base)?;
        }
        let (wt, w) = prepare_and_setup(st, host, job, &base)?;
        (Some(wt), w, Some(base))
    } else {
        if let Some(r) = &task.repo {
            if !host.dir_exists(ws, &dir) {
                return Err(format!("the repository folder '{r}' ({dir}) does not exist in the workspace"));
            }
        }
        // Senza worktree l'app non tocca git: legge solo la base per le istruzioni.
        let base = if delivery.uses_branches() { Some(task_base(host, job, &dir)?) } else { None };
        (None, Vec::new(), base)
    };
    let workdir = worktree.as_ref().map_or_else(|| dir.clone(), |w| w.path.clone());
    let permissions_file = host.worker_permissions_file(ws, &task.agent)?;
    let req = AutomationRequest {
        bootstrap_prompt: BOOTSTRAP_PROMPT,
        permission_mode: ws.worker.permission,
        model: task.model.as_deref().or(ws.worker.model.as_deref()),
        permissions_file: permissions_file.as_deref(),
    };
    let command = host.launch_command(&task.agent, ws.kind, &req)?;
    Ok(Prepared {
        worktree,
        warnings,
        new: NewWorker {
            workspace_id: ws.id.clone(),
            agent_id: task.agent.clone(),
            title: task.title.clone(),
            cwd: workdir.clone(),
            command,
            kind: task.kind,
        },
        env: WorkerEnv { delivery, repo_dir: dir, workdir, branch, base },
    })
}

/// Lavoro dell'avvio: git, comando di lancio, registrazione del worker e
/// spawn del tab. Gira su un thread dell'host; rientra nello stato solo per
/// registrare il risultato.
pub(in crate::automation) fn run_start_job<H: Host>(st: &AutomationState, host: &H, job: StartJob) {
    let (ws_id, tid, attempt) = (job.ws.id.clone(), job.task.id.clone(), job.attempt);
    // Un solo avvio per volta per task: stop + retry mentre il primo e'
    // ancora in git non devono sovrapporre due `worktree add` sullo stesso path.
    let key = format!("{ws_id}/{tid}");
    let start_lock = lock(&st.starts).entry(key.clone()).or_default().clone();
    {
        let _turn = lock(&start_lock);
        // Superato mentre aspettava il turno: niente git.
        if st.run(host, |r| r.start_is_current(&ws_id, &tid, attempt)) {
            match prepare_start(st, host, &job) {
                Ok(p) => {
                    let ev = st.run(host, |r| {
                        r.register_task_worker(&job, p.worktree, p.warnings, p.new, p.env, now_millis())
                    });
                    if let Some(ev) = ev {
                        host.emit_spawn(&ev);
                    }
                }
                Err(e) => st.run(host, |r| r.start_failed(&ws_id, &tid, attempt, &e, now_millis())),
            }
        }
    }
    let mut starts = lock(&st.starts);
    if starts.get(&key).is_some_and(|l| Arc::strong_count(l) <= 2) {
        starts.remove(&key);
    }
}
