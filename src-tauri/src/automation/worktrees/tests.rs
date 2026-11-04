//! Parti pure di `worktrees`: percorsi e interpretazione dell'elenco di git.

use super::reconcile::{parse_worktree_list, ListedWorktree, Reconciler};
use super::{is_abs, join_path, task_branch};
use crate::platform::TargetKind::{Windows, Wsl};
use std::sync::Arc;

#[test]
fn join_path_uses_the_separator_of_the_environment() {
    assert_eq!(join_path(Windows, "C:\\p", ".worktrees/a"), "C:\\p\\.worktrees\\a");
    assert_eq!(join_path(Wsl, "/home/u/p", ".worktrees/a"), "/home/u/p/.worktrees/a");
}

#[test]
fn join_path_trims_trailing_separators_of_the_root_only() {
    assert_eq!(join_path(Windows, "C:\\p\\", "a"), "C:\\p\\a");
    assert_eq!(join_path(Windows, "C:\\p/", "a"), "C:\\p\\a");
    assert_eq!(join_path(Wsl, "/home/u///", "a/b/"), "/home/u/a/b/");
    assert_eq!(join_path(Wsl, "/", "a"), "/a");
}

#[test]
fn join_path_keeps_spaces_and_unicode() {
    assert_eq!(join_path(Windows, "C:\\Mio progetto", "caffè/è"), "C:\\Mio progetto\\caffè\\è");
    assert_eq!(join_path(Wsl, "/home/u/mio progetto", "x y/z"), "/home/u/mio progetto/x y/z");
}

#[test]
fn is_abs_on_windows_accepts_drives_and_unc() {
    for p in ["C:\\p", "c:/p", "Z:\\", "\\\\wsl.localhost\\Ubuntu\\home", "//server/share"] {
        assert!(is_abs(Windows, p), "{p}");
    }
}

#[test]
fn is_abs_on_windows_rejects_relative_and_drive_relative_paths() {
    for p in ["", "p", ".git/info/exclude", "..\\p", "C:", "C:p", "1:\\p", "/home/u", "\\p"] {
        assert!(!is_abs(Windows, p), "{p}");
    }
}

#[test]
fn is_abs_on_wsl_only_accepts_rooted_paths() {
    assert!(is_abs(Wsl, "/"));
    assert!(is_abs(Wsl, "/home/u/p/.git/info/exclude"));
    for p in ["", "home/u", ".git/info/exclude", "C:\\p", "\\\\server\\share"] {
        assert!(!is_abs(Wsl, p), "{p}");
    }
}

const PORCELAIN: &str = "\
worktree C:/repo
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree C:/repo/.worktrees/a
HEAD 2222222222222222222222222222222222222222
branch refs/heads/task/a

worktree C:/repo/.worktrees/b
HEAD 3333333333333333333333333333333333333333
detached

worktree C:/repo/.worktrees/gone
HEAD 4444444444444444444444444444444444444444
branch refs/heads/task/gone
locked reason
prunable gitdir file points to non-existent location

worktree C:/bare.git
bare
";

fn listed(path: &str, branch: Option<&str>, prunable: bool) -> ListedWorktree {
    ListedWorktree { path: path.to_string(), branch: branch.map(str::to_string), prunable }
}

#[test]
fn porcelain_is_split_into_one_entry_per_worktree() {
    assert_eq!(
        parse_worktree_list(PORCELAIN),
        [
            listed("C:/repo", Some("main"), false),
            listed("C:/repo/.worktrees/a", Some("task/a"), false),
            listed("C:/repo/.worktrees/b", None, false),
            listed("C:/repo/.worktrees/gone", Some("task/gone"), true),
            listed("C:/bare.git", None, false),
        ]
    );
}

#[test]
fn porcelain_accepts_crlf_and_a_bare_prunable_marker() {
    let out = "worktree /r/.worktrees/a\r\nHEAD abc\r\nbranch refs/heads/task/a\r\nprunable\r\n";
    assert_eq!(parse_worktree_list(out), [listed("/r/.worktrees/a", Some("task/a"), true)]);
}

#[test]
fn porcelain_keeps_paths_with_spaces() {
    let out = "worktree /home/u/mio progetto/.worktrees/a\nbranch refs/heads/task/a\n";
    assert_eq!(parse_worktree_list(out)[0].path, "/home/u/mio progetto/.worktrees/a");
}

#[test]
fn porcelain_without_worktrees_is_empty() {
    assert!(parse_worktree_list("").is_empty());
    assert!(parse_worktree_list("\n\n").is_empty());
    // Righe prima del primo `worktree` non appartengono a nessuna voce.
    assert!(parse_worktree_list("branch refs/heads/task/a\n").is_empty());
}

#[test]
fn only_worktrees_made_by_the_app_have_a_task_id() {
    let id = |path: &str, branch: Option<&str>| listed(path, branch, false).task_id().map(str::to_string);
    assert_eq!(id("C:/repo/.worktrees/a", Some("task/a")), Some("a".to_string()));
    assert_eq!(id("C:\\repo\\.worktrees\\a-1", Some("task/a-1")), Some("a-1".to_string()));
    assert_eq!(id("/home/u/r/.worktrees/a/", Some("task/a")), Some("a".to_string()));
    // Il main checkout, un branch dell'utente, un detached HEAD.
    assert_eq!(id("C:/repo", Some("main")), None);
    assert_eq!(id("C:/repo/.worktrees/a", Some("feature/a")), None);
    assert_eq!(id("C:/repo/.worktrees/a", None), None);
    // Branch dei task, ma worktree altrove o con un nome diverso.
    assert_eq!(id("C:/elsewhere/a", Some("task/a")), None);
    assert_eq!(id("C:/repo/.worktrees/other", Some("task/a")), None);
    assert_eq!(id("C:/repo/worktrees/a", Some("task/a")), None);
    // Branch annidato: l'id sarebbe `a/b`, che non e' un nome di cartella.
    assert_eq!(id("C:/repo/.worktrees/b", Some("task/a/b")), None);
}

#[test]
fn task_branch_has_the_task_prefix() {
    assert_eq!(task_branch("fix-1"), "task/fix-1");
}

#[test]
fn sweeps_run_at_the_start_and_then_at_intervals_one_at_a_time() {
    let r = Arc::new(Reconciler::default());
    let first = r.begin_sweep(1_000).expect("primo giro all'avvio");
    // In corso: nemmeno dopo l'intervallo.
    assert!(r.begin_sweep(1_000 + 10 * 60_000).is_none());
    drop(first);
    // Finito, ma l'ultimo e' recente.
    assert!(r.begin_sweep(1_000 + 60_000).is_none());
    assert!(r.begin_sweep(1_000 + 5 * 60_000).is_some());
}

#[test]
fn a_clock_that_goes_back_does_not_stall_the_sweeps() {
    let r = Arc::new(Reconciler::default());
    drop(r.begin_sweep(10 * 60_000).unwrap());
    assert!(r.begin_sweep(10 * 60_000 - 1_000).is_none(), "poco indietro: ancora recente");
    assert!(r.begin_sweep(0).is_some(), "molto indietro: il giro riparte");
}

#[test]
fn a_dropped_sweep_turn_frees_the_slot_even_on_panic() {
    let r = Arc::new(Reconciler::default());
    let turn = r.begin_sweep(0).unwrap();
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _turn = turn;
        panic!("giro fallito");
    }));
    assert!(crashed.is_err());
    assert!(r.begin_sweep(10 * 60_000).is_some());
}

#[test]
fn one_cleanup_per_task_at_a_time() {
    let r = Reconciler::default();
    let a = r.begin_cleanup("ws-1", "a").expect("prima pulizia");
    assert!(r.begin_cleanup("ws-1", "a").is_none());
    assert!(r.begin_cleanup("ws-1", "b").is_some(), "un altro task");
    assert!(r.begin_cleanup("ws-2", "a").is_some(), "un altro workspace");
    drop(a);
    assert!(r.begin_cleanup("ws-1", "a").is_some());
}
