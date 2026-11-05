//! Pianificazione alla root: la cartella del workspace non e' un repository git
//! ma ha sottocartelle dirette che lo sono (piu' progetti nello stesso
//! workspace). Il pianificatore lavora nella cartella del workspace, senza
//! worktree ne' branch e in sola lettura, e ogni task che propone ha il `repo`
//! di uno dei repository rilevati. L'app fotografa lo stato git dei repository
//! all'avvio e lo confronta quando il tab del pianificatore si chiude: se e'
//! cambiato qualcosa il task riceve una nota e un avviso, ma nulla si annulla.

use crate::automation::worktrees::{git, is_git_repo, repo_dir};
use crate::automation::*;
use crate::board::{Task, TaskSpec};

/// Errore di un workspace senza nessun repository (lo legge il segretario).
pub(in crate::automation) const NO_REPOSITORY: &str =
    "this workspace has no git repository: neither its folder nor its direct subfolders; planning needs at least one. \
     Ask the user to run git init in the project folder";

/// Voce di `Host::list_repos` per la root stessa.
const ROOT_REPO: &str = ".";

/// Stato git di un repository: `HEAD` (vuoto senza commit) e `git status --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoState {
    repo: String,
    head: String,
    status: String,
}

/// Pianificatore alla root: i repository rilevati all'avvio e il loro stato di
/// allora (solo quelli letti senza errori).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootPlan {
    pub(in crate::automation) repos: Vec<String>,
    before: Vec<RepoState>,
}

/// Controllo di sola lettura da fare quando il tab del pianificatore si chiude.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyCheck {
    ws_id: String,
    task_id: String,
    attempt: u32,
    plan: RootPlan,
}

/// Repository in cui pianificare alla root: `None` se la root e' un repository
/// git (si pianifica nel suo worktree), altrimenti le sottocartelle git.
/// Errore se non ce n'e' nessuno o git non e' disponibile. Fuori dal lock.
pub(in crate::automation) fn root_repos<H: Host>(host: &H, ws: &Workspace) -> Result<Option<Vec<String>>, String> {
    if is_git_repo(host, ws, &ws.path)? {
        return Ok(None);
    }
    let repos: Vec<String> = host.list_repos(ws)?.into_iter().filter(|r| r != ROOT_REPO).collect();
    if repos.is_empty() {
        return Err(NO_REPOSITORY.to_string());
    }
    Ok(Some(repos))
}

/// All'avvio di `task`: il pianificatore alla root, se il task e' un piano
/// della root senza worktree (un retry con il worktree lo riusa) e la root non
/// e' un repository git. Fotografa lo stato dei repository. Fuori dal lock.
pub(in crate::automation) fn start<H: Host>(host: &H, ws: &Workspace, task: &Task) -> Result<Option<RootPlan>, String> {
    if !task.kind.isolated() || task.repo.is_some() || task.worktree.is_some() {
        return Ok(None);
    }
    let Some(repos) = root_repos(host, ws)? else {
        return Ok(None);
    };
    let before = snapshot(host, ws, &repos);
    Ok(Some(RootPlan { repos, before }))
}

/// Stato dei repository; uno che non si legge si scrive nei log e si salta.
fn snapshot<H: Host>(host: &H, ws: &Workspace, repos: &[String]) -> Vec<RepoState> {
    repos
        .iter()
        .filter_map(|repo| {
            repo_state(host, ws, repo)
                .inspect_err(|e| log::warn!("workspace {}: state of repository {repo} not read: {e}", ws.id))
                .ok()
        })
        .collect()
}

fn repo_state<H: Host>(host: &H, ws: &Workspace, repo: &str) -> Result<RepoState, String> {
    let dir = repo_dir(ws, Some(repo));
    let head = host.git(ws, &dir, &["rev-parse", "HEAD"]).map_err(|e| format!("git is not available: {e}"))?;
    // Senza commit `rev-parse HEAD` fallisce: HEAD vuoto.
    let head = if head.ok { head.stdout.trim().to_string() } else { String::new() };
    let status = git(host, ws, &dir, &["status", "--porcelain", "--untracked-files=all"])?;
    Ok(RepoState { repo: repo.to_string(), head, status })
}

/// Repository il cui stato e' cambiato; quelli non letti a una delle due
/// fotografie non si confrontano.
fn changed_repos(before: &[RepoState], after: &[RepoState]) -> Vec<String> {
    before.iter().filter(|b| after.iter().any(|a| a.repo == b.repo && a != *b)).map(|b| b.repo.clone()).collect()
}

/// Nota del task di un pianificatore che ha cambiato dei repository (la legge l'utente).
pub(in crate::automation) fn changes_note(repos: &[String]) -> String {
    format!("il pianificatore ha modificato file in: {} (doveva lavorare in sola lettura)", repos.join(", "))
}

/// Ogni task di una proposta del pianificatore alla root ha il `repo` di uno
/// dei repository rilevati (la root non e' un repository).
pub(in crate::automation) fn check_proposal_repos(specs: &[TaskSpec], repos: &[String]) -> Result<(), String> {
    let list = repos.join(", ");
    for s in specs {
        match s.repo.as_deref() {
            None => {
                return Err(format!(
                    "task '{}': repo is required. The workspace root is not a git repository: every proposed task \
                     must set repo to one of the detected repositories ({list})",
                    s.id
                ))
            }
            Some(r) if !repos.iter().any(|x| x == r) => {
                return Err(format!("task '{}': repo '{r}' is not one of the detected repositories ({list})", s.id))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Controllo di sola lettura (thread dell'host): rilegge lo stato dei
/// repository e, se e' cambiato, lo annota nel task.
pub(in crate::automation) fn run_read_only_check<H: Host>(st: &AutomationState, host: &H, check: ReadOnlyCheck) {
    if check.plan.before.is_empty() {
        return;
    }
    let Some(ws) = host.workspace(&check.ws_id) else {
        log::warn!("read-only check of task {}/{}: workspace not found", check.ws_id, check.task_id);
        return;
    };
    let after = snapshot(host, &ws, &check.plan.repos);
    let changed = changed_repos(&check.plan.before, &after);
    if !changed.is_empty() {
        st.run(host, |r| r.root_plan_changed(&check.ws_id, &check.task_id, check.attempt, &changed, now_millis()));
    }
}

impl Registry {
    /// Il tab del worker `session` si e' chiuso: se era un pianificatore alla
    /// root, il controllo di sola lettura va eseguito (una volta sola).
    pub(in crate::automation) fn queue_read_only_check(&mut self, session: &str) {
        let Some(w) = self.workers.get_mut(session) else {
            return;
        };
        if let Some(plan) = w.env.root_plan.take() {
            let check =
                ReadOnlyCheck { ws_id: w.workspace_id.clone(), task_id: w.task_id.clone(), attempt: w.attempt, plan };
            self.effects.push(Effect::ReadOnlyCheck(Box::new(check)));
        }
    }

    /// Il pianificatore del tentativo `attempt` ha cambiato `repos`: nota e avviso.
    pub fn root_plan_changed(&mut self, ws: &str, tid: &str, attempt: u32, repos: &[String], now: i64) {
        let Some(t) = self.boards.get_mut(ws).and_then(|b| b.task_mut(tid)).filter(|t| t.attempts() == attempt) else {
            return;
        };
        if t.append_note(&changes_note(repos), now) {
            self.mark_dirty(ws);
            self.notify(ws, tid, None, "attention");
        }
    }
}

impl AutomationState {
    /// Repository del pianificatore alla root `session`, se lo e'.
    pub(in crate::automation) fn root_repos_of(&self, session: &str) -> Option<Vec<String>> {
        let reg = self.registry();
        reg.workers.get(session)?.env.root_plan.as_ref().map(|p| p.repos.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(repo: &str, head: &str, status: &str) -> RepoState {
        RepoState { repo: repo.into(), head: head.into(), status: status.into() }
    }

    #[test]
    fn only_repositories_read_twice_and_different_are_changed() {
        let before = [state("api", "a1", ""), state("web", "b1", ""), state("docs", "c1", "")];
        let after = [state("api", "a2", ""), state("web", "b1", " M x.ts\n")];
        assert_eq!(changed_repos(&before, &after), ["api", "web"], "docs not read at the end: not compared");
        assert!(changed_repos(&before, &before).is_empty());
        assert!(changed_repos(&[], &after).is_empty());
    }

    #[test]
    fn a_root_proposal_names_a_detected_repository_for_every_task() {
        let repos = ["api".to_string(), "web".to_string()];
        let spec = |id: &str, repo: Option<&str>| TaskSpec {
            id: id.into(),
            repo: repo.map(str::to_string),
            ..TaskSpec::default()
        };
        assert_eq!(check_proposal_repos(&[spec("a", Some("api")), spec("b", Some("web"))], &repos), Ok(()));
        let e = check_proposal_repos(&[spec("a", Some("api")), spec("b", None)], &repos).unwrap_err();
        assert!(e.starts_with("task 'b': repo is required") && e.contains("(api, web)"), "{e}");
        let e = check_proposal_repos(&[spec("c", Some("other"))], &repos).unwrap_err();
        assert_eq!(e, "task 'c': repo 'other' is not one of the detected repositories (api, web)");
    }

    #[test]
    fn the_note_names_the_changed_repositories() {
        assert_eq!(
            changes_note(&["backend".into(), "frontend".into()]),
            "il pianificatore ha modificato file in: backend, frontend (doveva lavorare in sola lettura)"
        );
    }
}
