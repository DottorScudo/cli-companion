//! Modalita' libera: la cartella del task non e' un repository git. Il task
//! gira sul posto, senza worktree, branch, commit o PR, qualunque sia la
//! consegna del workspace; git resta com'e' per le cartelle git.

use super::e2e::e2e_repo;
use super::*;
use crate::automation::planner::create_hub_plan_task;
use crate::automation::planner::{create_plan_task, PlanRequest};

const FREE_LABEL: &str = "free mode (the folder is not a git repository)";
const MODES: [(DeliveryMode, bool); 8] = [
    (DeliveryMode::Pr, true),
    (DeliveryMode::Pr, false),
    (DeliveryMode::LocalMerge, true),
    (DeliveryMode::LocalMerge, false),
    (DeliveryMode::Push, true),
    (DeliveryMode::Push, false),
    (DeliveryMode::Commit, true),
    (DeliveryMode::Commit, false),
];

/// Workspace "ws-1" la cui root `C:\p` non e' un repository git.
fn free_host(mode: DeliveryMode, parallel: bool) -> FakeHost {
    let h = FakeHost::with_delivery(mode, parallel);
    not_git(&h, "C:\\p");
    h
}

fn not_git(h: &FakeHost, dir: &str) {
    h.git_rule(&format!("{dir}|rev-parse --is-inside-work-tree"), false, "", "fatal: not a git repository");
}

fn spec_in(id: &str, repo: &str) -> Value {
    let mut s = spec(id, &[]);
    s["repo"] = json!(repo);
    s
}

fn status_of(st: &AutomationState, h: &FakeHost, id: &str) -> String {
    task(st, h, id)["status"].as_str().unwrap_or("missing").to_string()
}

fn cwd_of(h: &FakeHost, tab: &str) -> Option<String> {
    h.spawns().into_iter().find(|s| s.tab_id == tab).and_then(|s| s.cwd)
}

/// Comandi git che toccano worktree, branch o rete: in modalita' libera non ce ne sono.
fn branch_git(h: &FakeHost) -> Vec<String> {
    let forbidden = ["worktree", "branch", "fetch", "symbolic-ref", "abbrev-ref", "switch", "merge", "push"];
    h.git_log().into_iter().filter(|l| forbidden.iter().any(|f| l.contains(f))).collect()
}

#[test]
fn a_task_in_a_root_without_git_starts_in_place_in_every_delivery_mode() {
    for (mode, parallel) in MODES {
        let (st, h) = (AutomationState::default(), free_host(mode, parallel));
        queue(&st, &h, vec![spec("a", &[])]);
        let t = task(&st, &h, "a");
        assert_eq!(t["status"], "running", "{mode:?} {parallel}: {t}");
        assert_eq!(t["no_git"], true, "{t}");
        assert!(t["worktree"].is_null(), "{t}");
        assert_eq!(cwd_of(&h, t["session"].as_str().unwrap()).as_deref(), Some("C:\\p"));
        assert!(branch_git(&h).is_empty(), "{mode:?} {parallel}: {:?}", h.git_log());
        assert!(h.hook_log.lock().unwrap().is_empty() && h.excludes.lock().unwrap().is_empty());
    }
}

#[test]
fn the_flag_is_recorded_at_creation_and_confirmed_at_the_start() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::Pr, true));
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    let manual = json!({"title": "Manual", "task": "x", "model": "sonnet"});
    boards::task_create(&st, &h, "ws-1", &manual).unwrap();
    let t = task(&st, &h, "manual");
    assert_eq!((t["status"].as_str(), t["no_git"].as_bool()), (Some("queued"), Some(true)), "{t}");
    // Il rilevamento all'avvio e' quello che vale: la cartella e' diventata git.
    h.git_rules.lock().unwrap().clear();
    boards::board_set(&st, &h, "ws-1", None, Some(false)).unwrap();
    let t = task(&st, &h, "manual");
    assert_eq!(t["status"], "running", "{t}");
    assert!(t["no_git"].is_null(), "git again: the flag is gone: {t}");
    assert!(t["worktree"]["path"].as_str().unwrap().ends_with(".worktrees\\manual"), "{t}");
}

#[test]
fn a_free_task_reads_free_mode_texts_and_completes_without_git() {
    for (mode, parallel) in MODES {
        let (st, h) = (AutomationState::default(), free_host(mode, parallel));
        queue(&st, &h, vec![spec("a", &[])]);
        let w = session_of(&st, &h, "a");
        let t = call(&st, &h, &w, "get_task", json!({})).unwrap();
        let instr = t["instructions"].as_str().unwrap();
        assert!(instr.contains("This folder is not a git repository: work directly in it"), "{instr}");
        assert!(instr.contains("Work only inside C:\\p;") && !instr.contains("Commit only"), "{instr}");
        let env = t["environment"].as_str().unwrap();
        assert!(env.contains(&format!("- Delivery mode: {FREE_LABEL}")), "{env}");
        assert!(env.contains("- Folder: C:\\p (not a git repository)") && !env.contains("Task branch"), "{env}");
        let delivery = t["delivery"].as_str().unwrap();
        assert!(delivery.starts_with(&format!("Delivery: {FREE_LABEL}")), "{delivery}");
        assert!(!delivery.contains("git rebase") && !delivery.contains("git push") && !delivery.contains("`commit`"));

        let summary = "created notes.txt";
        call(&st, &h, &w, "complete_task", json!({"status": "completed", "summary": summary})).unwrap();
        let t = task(&st, &h, "a");
        assert_eq!(t["status"], "done", "{t}");
        assert!(t["report"]["diffstat"].is_null() && t["report"]["commit"].is_null(), "{t}");
        assert!(t["note"].is_null(), "{t}");
        tab_closed(&st, &h, &w);
        assert!(branch_git(&h).is_empty(), "{mode:?} {parallel}: {:?}", h.git_log());
        assert!(!h.git_log().iter().any(|l| l.contains("diff --stat")), "no diffstat");
    }
}

#[test]
fn two_tasks_of_the_same_folder_without_git_never_run_together_even_in_parallel() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::LocalMerge, true));
    queue(&st, &h, vec![spec("a", &[]), spec("b", &[])]);
    assert_eq!((status_of(&st, &h, "a"), status_of(&st, &h, "b")), ("running".into(), "queued".into()));
    finish(&st, &h, &session_of(&st, &h, "a"), "completed");
    assert_eq!((status_of(&st, &h, "a"), status_of(&st, &h, "b")), ("done".into(), "running".into()));
}

#[test]
fn a_mixed_workspace_runs_the_root_free_and_the_git_repository_in_a_worktree() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::LocalMerge, true));
    h.dirs.lock().unwrap().push("C:\\p\\api".to_string());
    queue(&st, &h, vec![spec("r", &[]), spec_in("g", "api")]);
    assert_eq!((status_of(&st, &h, "r"), status_of(&st, &h, "g")), ("running".into(), "running".into()));
    let (root, api) = (task(&st, &h, "r"), task(&st, &h, "g"));
    assert_eq!((root["no_git"].as_bool(), api["no_git"].is_null()), (Some(true), true));
    assert_eq!(cwd_of(&h, root["session"].as_str().unwrap()).as_deref(), Some("C:\\p"));
    assert_eq!(cwd_of(&h, api["session"].as_str().unwrap()).as_deref(), Some("C:\\p\\api\\.worktrees\\g"));
    let git = h.git_log();
    assert!(git.contains(&"C:\\p\\api|worktree add C:\\p\\api\\.worktrees\\g -b task/g main".to_string()), "{git:?}");
    assert!(!git.iter().any(|l| l.starts_with("C:\\p|") && l.contains("worktree")), "{git:?}");
}

#[test]
fn an_e2e_task_is_refused_in_a_folder_without_git() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::Pr, true));
    h.repo_cfg.lock().unwrap().insert(".".into(), e2e_repo());
    let mut flagged = spec("a", &[]);
    flagged["e2e"] = json!(true);
    let needle = "task 'a': e2e needs a git repository: C:\\p is not one";
    let e = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [flagged.clone()]})).unwrap_err();
    assert!(e.contains(needle) && e.contains("Remove e2e"), "{e}");
    let mut manual = json!({"title": "A", "task": "x", "model": "sonnet", "e2e": true});
    assert!(boards::task_create(&st, &h, "ws-1", &manual).unwrap_err().contains("e2e needs a git repository"));
    manual["e2e"] = json!(false);
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    boards::task_create(&st, &h, "ws-1", &manual).unwrap();
    let e = boards::task_update(&st, &h, "ws-1", "a", &json!({"e2e": true})).unwrap_err();
    assert!(e.contains("e2e needs a git repository"), "{e}");
    assert!(task(&st, &h, "a")["e2e"].is_null());
}

#[test]
fn an_e2e_task_whose_folder_lost_git_fails_to_start_clearly() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Pr, true));
    h.repo_cfg.lock().unwrap().insert(".".into(), e2e_repo());
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    let manual = json!({"title": "A", "task": "x", "model": "sonnet", "e2e": true});
    boards::task_create(&st, &h, "ws-1", &manual).unwrap();
    not_git(&h, "C:\\p");
    boards::board_set(&st, &h, "ws-1", None, Some(false)).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!(t["status"], "failed", "{t}");
    assert!(t["note"].as_str().unwrap().contains("e2e needs a git repository"), "{t}");
}

#[test]
fn a_plan_of_a_root_without_git_or_repositories_runs_in_place() {
    for (mode, parallel) in MODES {
        let (st, h) = (AutomationState::default(), free_host(mode, parallel));
        let id = create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan the work").unwrap();
        let t = task(&st, &h, &id);
        assert_eq!((t["status"].as_str(), t["no_git"].as_bool()), (Some("running"), Some(true)), "{t}");
        assert!(t["worktree"].is_null(), "{t}");
        let p = t["session"].as_str().unwrap().to_string();
        assert_eq!(cwd_of(&h, &p).as_deref(), Some("C:\\p"));
        assert!(branch_git(&h).is_empty(), "{:?}", h.git_log());
        let t = call(&st, &h, &p, "get_task", json!({})).unwrap();
        let instr = t["instructions"].as_str().unwrap();
        assert!(instr.contains("NOT a git repository") && instr.contains("free mode"), "{instr}");
        assert!(instr.contains("directly inside it (folders relative to the root): none."), "{instr}");
        let env = t["environment"].as_str().unwrap();
        assert!(env.contains("- Workspace root: C:\\p (not a git repository)"), "{env}");
        assert!(env.contains("runs in free mode") && env.contains("Read-only"), "{env}");
        // Senza `repo` (la root) o in una cartella senza git: valide.
        h.dirs.lock().unwrap().push("C:\\p\\docs".to_string());
        not_git(&h, "C:\\p\\docs");
        let tasks = json!([spec("a", &[]), spec_in("b", "docs")]);
        assert_eq!(call(&st, &h, &p, "propose_tasks", json!({ "tasks": tasks })).unwrap()["status"], "pending");
    }
}

#[test]
fn a_plan_of_a_subfolder_without_git_runs_in_place_and_one_of_a_git_subfolder_in_a_worktree() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::LocalMerge, true));
    for dir in ["C:\\p\\docs", "C:\\p\\api"] {
        h.dirs.lock().unwrap().push(dir.to_string());
    }
    not_git(&h, "C:\\p\\docs");
    let req = |repo: &str| PlanRequest {
        title: format!("Plan {repo}"),
        task: "Plan it".into(),
        model: None,
        repo: Some(repo.into()),
    };
    let docs = create_plan_task(&st, &h, "ws-1", &req("docs")).unwrap();
    let p = session_of(&st, &h, &docs);
    assert_eq!(cwd_of(&h, &p).as_deref(), Some("C:\\p\\docs"));
    assert!(task(&st, &h, &docs)["worktree"].is_null());
    let t = call(&st, &h, &p, "get_task", json!({})).unwrap();
    assert!(t["instructions"].as_str().unwrap().contains("It is the folder `docs` of the workspace root."));
    let env = t["environment"].as_str().unwrap();
    assert!(env.contains("docs (a folder of the workspace root, not a git repository)"), "{env}");
    assert!(env.contains("no worktree and no branch. Read-only"), "{env}");
    let api = create_plan_task(&st, &h, "ws-1", &req("api")).unwrap();
    let wt = task(&st, &h, &api)["worktree"]["path"].clone();
    assert_eq!(wt, "C:\\p\\api\\.worktrees\\plan-api");
    assert!(task(&st, &h, &api)["no_git"].is_null());
}

#[test]
fn a_free_folder_is_listed_with_its_flag() {
    let (st, h) = (AutomationState::default(), free_host(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    let r = call(&st, &h, INTAKE, "list_tasks", json!({})).unwrap();
    assert_eq!(r["tasks"][0]["no_git"], true, "{r}");
    let r = call(&st, &h, INTAKE, "get_task_details", json!({"id": "a"})).unwrap();
    assert_eq!(r["task"]["no_git"], true, "{r}");
}
