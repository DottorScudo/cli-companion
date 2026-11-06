//! Pianificazione alla root senza git (workspace con piu' repository nelle
//! sottocartelle): creazione, avvio senza worktree, cosa legge il
//! pianificatore, proposta con `repo`, controllo di sola lettura e pulizia.

use super::*;
use crate::automation::planner::create_hub_plan_task;
use crate::automation::planner::root::changes_note;
use crate::board::texts::PLANNER_ROOT;

const REPOS: [&str; 2] = ["backend", "frontend"];

/// Workspace "ws-1" con la root `C:\p` che non e' un repository git e due
/// sottocartelle git. `local_merge` con il checkout su un altro branch: un
/// pianificatore alla root non deve passare dai controlli sul checkout.
fn root_host() -> FakeHost {
    let h = FakeHost::with_delivery(DeliveryMode::LocalMerge, false);
    *h.branch.lock().unwrap() = Some("feature".into());
    h.git_rule("C:\\p|rev-parse --is-inside-work-tree", false, "", "fatal: not a git repository");
    *h.repos.lock().unwrap() = REPOS.iter().map(|r| r.to_string()).collect();
    h.dirs.lock().unwrap().extend(REPOS.iter().map(|r| format!("C:\\p\\{r}")));
    h
}

/// Pianificazione aperta dall'HUB e partita: (stato, host, id, sessione).
fn root_planner() -> (AutomationState, FakeHost, String, String) {
    let (st, h) = (AutomationState::default(), root_host());
    let id = create_hub_plan_task(&st, &h, "ws-1", "Plan login", "Add a login to both apps", "general").unwrap();
    let session = session_of(&st, &h, &id);
    (st, h, id, session)
}

/// Da ora `git status` del repository `repo` risponde `status`.
fn repo_status_becomes(h: &FakeHost, repo: &str, status: &str) {
    let out = GitOut { ok: true, stdout: status.to_string(), stderr: String::new() };
    h.git_rules.lock().unwrap().insert(0, (format!("C:\\p\\{repo}|status --porcelain"), out));
}

fn spec_in(id: &str, repo: Option<&str>) -> Value {
    let mut s = spec(id, &[]);
    s["repo"] = repo.map_or(Value::Null, |r| json!(r));
    s
}

fn attention_notices(h: &FakeHost) -> usize {
    h.notices().iter().filter(|n| n.kind == "attention").count()
}

#[test]
fn a_root_plan_starts_in_the_workspace_folder_without_worktree() {
    let (st, h, id, p) = root_planner();
    let t = task(&st, &h, &id);
    assert_eq!((t["kind"].as_str(), t["status"].as_str()), (Some("plan"), Some("running")), "{t}");
    assert!(t["worktree"].is_null() && t["repo"].is_null(), "{t}");
    let spawn = h.spawns().into_iter().find(|s| s.tab_id == p).unwrap();
    assert_eq!(spawn.cwd.as_deref(), Some("C:\\p"));
    let git = h.git_log();
    for forbidden in ["worktree", "abbrev-ref", "fetch", "branch"] {
        assert!(!git.iter().any(|l| l.contains(forbidden)), "{forbidden}: {git:?}");
    }
    for repo in REPOS {
        assert!(git.contains(&format!("C:\\p\\{repo}|rev-parse HEAD")), "{git:?}");
        assert!(git.contains(&format!("C:\\p\\{repo}|status --porcelain --untracked-files=all")), "{git:?}");
    }
    assert!(h.hook_log.lock().unwrap().is_empty(), "no setup");
}

#[test]
fn get_task_of_a_root_plan_names_the_repositories_and_the_read_only_rule() {
    let (st, h, _, p) = root_planner();
    let t = call(&st, &h, &p, "get_task", json!({})).unwrap();
    let instr = t["instructions"].as_str().unwrap();
    assert!(instr.starts_with("You are an automated cli-companion planner. Work only inside C:\\p;"), "{instr}");
    let place = "It is the workspace root. Git repositories directly inside it (folders relative to the root): backend, frontend.";
    let section = PLANNER_ROOT.trim_end().replace("{place}", place);
    assert!(instr.contains(&section), "{instr}");
    assert!(instr.contains("never create, modify or delete files") && instr.contains("free mode"), "{instr}");
    let env = t["environment"].as_str().unwrap();
    assert!(env.contains("- Workspace root: C:\\p (not a git repository)"), "{env}");
    assert!(env.contains("(folders relative to the root): backend, frontend."), "{env}");
    assert!(!env.contains("Git worktree") && !env.contains("Repository checkout"), "{env}");
    assert!(t.get("delivery").is_none(), "{t}");
}

#[test]
fn a_root_planner_may_leave_repo_unset_or_name_any_existing_folder() {
    let (st, h, id, p) = root_planner();
    h.dirs.lock().unwrap().push("C:\\p\\docs".to_string());
    let tasks = json!([spec_in("ok", Some("backend")), spec_in("gone", Some("nowhere"))]);
    let e = call(&st, &h, &p, "propose_tasks", json!({ "tasks": tasks })).unwrap_err();
    assert!(e.contains("'nowhere'") && e.contains("does not exist"), "{e}");
    assert!(view(&st, &h).proposals.is_empty(), "a rejected proposal does not count");
    let mut web = spec_in("web", Some("frontend"));
    web["depends_on"] = json!(["api"]);
    let tasks = json!([spec_in("root", None), spec_in("api", Some("backend")), spec_in("docs", Some("docs")), web]);
    assert_eq!(call(&st, &h, &p, "propose_tasks", json!({ "tasks": tasks })).unwrap()["status"], "pending");
    let proposals = view(&st, &h).proposals;
    assert_eq!(proposals[0].source_task.as_deref(), Some(id.as_str()));
    let repos: Vec<_> = proposals[0].tasks.iter().map(|t| t.repo.as_deref()).collect();
    assert_eq!(repos, [None, Some("backend"), Some("docs"), Some("frontend")]);
}

#[test]
fn the_intake_of_a_root_workspace_can_queue_tasks_for_the_root() {
    let (st, h, _, _) = root_planner();
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    let r = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [spec_in("a", None)]})).unwrap();
    assert_eq!(r["status"], "pending");
}

#[test]
fn a_planner_that_changes_a_repository_gets_a_note_and_an_attention() {
    let (st, h, id, p) = root_planner();
    call(&st, &h, &p, "get_task", json!({})).unwrap();
    repo_status_becomes(&h, "backend", " M src/main.rs\n");
    call(&st, &h, &p, "complete_task", json!({"status": "completed", "summary": "proposed"})).unwrap();
    assert!(task(&st, &h, &id)["note"].is_null(), "checked when the tab closes");
    tab_closed(&st, &h, &p);
    let t = task(&st, &h, &id);
    assert_eq!(t["status"], "done", "the plan stays concluded");
    assert_eq!(t["note"].as_str(), Some(changes_note(&["backend".to_string()]).as_str()));
    assert_eq!(attention_notices(&h), 1);
    let git = h.git_log();
    assert!(!git.iter().any(|l| l.contains("worktree") || l.contains("branch")), "nothing to clean: {git:?}");
    assert!(!git.iter().any(|l| l.contains("diff --stat") || l.contains("checkout") || l.contains("reset")), "{git:?}");
    // Una seconda chiusura non ripete il controllo.
    tab_closed(&st, &h, &p);
    assert_eq!(attention_notices(&h), 1);
}

#[test]
fn a_failed_or_abandoned_root_plan_is_checked_too() {
    let (st, h, id, p) = root_planner();
    call(&st, &h, &p, "get_task", json!({})).unwrap();
    h.git_rule("C:\\p\\frontend|rev-parse HEAD", true, "a1b2c3\n", "");
    tab_closed(&st, &h, &p);
    let t = task(&st, &h, &id);
    assert_eq!(t["status"], "failed");
    let note = t["note"].as_str().unwrap();
    assert!(note.starts_with("the tab was closed before complete_task; "), "{note}");
    assert!(note.ends_with(&changes_note(&["frontend".to_string()])), "{note}");

    let (st, h, id, p) = root_planner();
    call(&st, &h, &p, "get_task", json!({})).unwrap();
    repo_status_becomes(&h, "backend", "?? notes.md\n");
    repo_status_becomes(&h, "frontend", " D old.ts\n");
    call(&st, &h, &p, "complete_task", json!({"status": "failed", "summary": "cannot plan"})).unwrap();
    tab_closed(&st, &h, &p);
    let t = task(&st, &h, &id);
    assert_eq!(t["status"], "failed");
    assert_eq!(t["note"].as_str(), Some(changes_note(&["backend".to_string(), "frontend".to_string()]).as_str()));
}

#[test]
fn an_untouched_or_unreadable_repository_gives_no_note() {
    let (st, h, id, p) = root_planner();
    call(&st, &h, &p, "get_task", json!({})).unwrap();
    call(&st, &h, &p, "complete_task", json!({"status": "completed", "summary": "proposed"})).unwrap();
    tab_closed(&st, &h, &p);
    assert!(task(&st, &h, &id)["note"].is_null());
    assert_eq!(attention_notices(&h), 0);

    // Lo stato non si rilegge alla fine: scritto nei log, il task si conclude com'e'.
    let (st, h, id, p) = root_planner();
    call(&st, &h, &p, "get_task", json!({})).unwrap();
    h.git_rules.lock().unwrap().insert(
        0,
        ("status --porcelain".to_string(), GitOut { ok: false, stdout: String::new(), stderr: "boom".into() }),
    );
    call(&st, &h, &p, "complete_task", json!({"status": "completed", "summary": "proposed"})).unwrap();
    tab_closed(&st, &h, &p);
    let t = task(&st, &h, &id);
    assert_eq!((t["status"].as_str(), t["note"].as_str()), (Some("done"), None));
    assert_eq!(attention_notices(&h), 0);
}

#[test]
fn a_root_without_any_repository_is_planned_in_place_too() {
    let (st, h) = (AutomationState::default(), root_host());
    // Un repository "." non conta: e' la root, che non risponde come repository git.
    *h.repos.lock().unwrap() = vec![".".to_string()];
    let id = create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "general").unwrap();
    let p = session_of(&st, &h, &id);
    let t = call(&st, &h, &p, "get_task", json!({})).unwrap();
    assert!(t["instructions"].as_str().unwrap().contains("directly inside it (folders relative to the root): none."));
    assert!(t["environment"]
        .as_str()
        .unwrap()
        .contains("repositories of the workspace (folders relative to the root): none."));
    assert!(!h.git_log().iter().any(|l| l.contains("worktree")), "{:?}", h.git_log());
}

#[test]
fn repositories_that_cannot_be_listed_at_the_start_fail_the_plan_clearly() {
    let (st, h) = (AutomationState::default(), root_host());
    h.hold_defers();
    let id = create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "general").unwrap();
    *h.repos_err.lock().unwrap() = Some("wsl unavailable".to_string());
    h.run_held();
    let t = task(&st, &h, &id);
    assert_eq!((t["status"].as_str(), t["note"].as_str()), (Some("failed"), Some("wsl unavailable")), "{t}");
}

#[test]
fn a_git_root_keeps_planning_in_its_worktree() {
    let h = FakeHost::with_delivery(DeliveryMode::Commit, false);
    *h.repos.lock().unwrap() = vec![".".to_string(), "backend".to_string()];
    let st = AutomationState::default();
    let id = create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "general").unwrap();
    let p = session_of(&st, &h, &id);
    assert_eq!(task(&st, &h, &id)["worktree"]["path"], "C:\\p\\.worktrees\\plan");
    let t = call(&st, &h, &p, "get_task", json!({})).unwrap();
    assert!(!t["instructions"].as_str().unwrap().contains("Workspace root without git"));
    // Senza la regola della root: un task senza `repo` (la root) si propone.
    call(&st, &h, &p, "propose_tasks", json!({"tasks": [spec_in("a", None)]})).unwrap();
}
