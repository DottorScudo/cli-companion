//! Pianificatore sul posto: il piano di una cartella che non e' un repository
//! git (la root del workspace, che magari contiene sottocartelle git, o una sua
//! sottocartella). Il pianificatore lavora nella cartella stessa, senza
//! worktree ne' branch e in sola lettura. L'app fotografa lo stato git dei
//! repository rilevati (quelli diretti della root) all'avvio e lo confronta
//! quando il tab del pianificatore si chiude: se e' cambiato qualcosa il task
//! riceve una nota e un avviso, ma nulla si annulla.

use crate::automation::worktrees::{git, repo_dir};
use crate::automation::*;
use crate::board::Task;

/// Voce di `Host::list_repos` per la root stessa.
const ROOT_REPO: &str = ".";

/// Stato git di un repository: `HEAD` (vuoto senza commit) e `git status --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoState {
    repo: String,
    head: String,
    status: String,
}

/// Pianificatore sul posto: i repository git rilevati all'avvio (vuoti se la
/// cartella non e' la root) e il loro stato di allora (solo quelli letti senza
/// errori).
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

/// Il pianificatore sul posto di `task`, un piano di una cartella senza git:
/// i repository git rilevati (solo se la cartella e' la root del workspace,
/// che li contiene) e il loro stato di adesso. Fuori dal lock.
pub(in crate::automation) fn start<H: Host>(host: &H, ws: &Workspace, task: &Task) -> Result<RootPlan, String> {
    let repos = if task.repo.is_none() {
        host.list_repos(ws)?.into_iter().filter(|r| r != ROOT_REPO).collect()
    } else {
        Vec::new()
    };
    let before = snapshot(host, ws, &repos);
    Ok(RootPlan { repos, before })
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
    fn the_note_names_the_changed_repositories() {
        assert_eq!(
            changes_note(&["backend".into(), "frontend".into()]),
            "il pianificatore ha modificato file in: backend, frontend (doveva lavorare in sola lettura)"
        );
    }
}
