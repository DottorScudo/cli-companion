//! Testi del pianificatore sul posto in una cartella senza git (la root del
//! workspace o una sua sottocartella): dove lavora e in che ambiente.

use super::EnvInfo;

/// Dove lavora il pianificatore sul posto: la root (con i repository git
/// rilevati) o una sottocartella senza git.
pub(super) fn in_place_description(e: &EnvInfo, repos: &[String]) -> String {
    match &e.task.repo {
        Some(r) => format!("It is the folder `{r}` of the workspace root."),
        None => format!(
            "It is the workspace root. Git repositories directly inside it (folders relative to the root): {}.",
            repos_list(repos)
        ),
    }
}

fn repos_list(repos: &[String]) -> String {
    if repos.is_empty() {
        "none".to_string()
    } else {
        repos.join(", ")
    }
}

/// `environment` del pianificatore sul posto in una cartella senza git: la
/// root (con i repository rilevati) o una sottocartella, nessun worktree.
pub(super) fn root_planner_environment(e: &EnvInfo, repos: &[String]) -> String {
    let mut s = String::from("Environment of this task\n");
    match &e.task.repo {
        Some(r) => s.push_str(&format!(
            "- Workspace root: {}\n- Folder: {r} (a folder of the workspace root, not a git repository)\n",
            e.root
        )),
        None => s.push_str(&format!(
            "- Workspace root: {root} (not a git repository)\n\
             - Git repositories of the workspace (folders relative to the root): {list}.\n\
             - A proposed task may set `repo` to one of the git repositories (it runs in a git worktree as usual), to another existing folder of the root, or leave it unset for the root itself.\n\
             - Every folder that is not a git repository (the root included) runs in free mode: the worker works directly in the folder, with no worktree, branch or commit.\n",
            root = e.root,
            list = repos_list(repos)
        )),
    }
    s.push_str(&format!(
        "- Your working folder: {}, not a git repository: no worktree and no branch. Read-only: do not modify anything.\n",
        e.workdir
    ));
    s
}
