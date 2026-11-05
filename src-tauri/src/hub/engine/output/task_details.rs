//! Un task per intero, per `task_details` (puro): lo stesso JSON del task
//! (`TaskView::to_json`), senza i campi interni (sessione, percorso del
//! worktree, storico delle domande) e con i testi tagliati perche' l'esito
//! stia in `TOOL_RESULT_MAX`; cio' che e' tagliato e' elencato in `truncated`.
//! Specifica, nota, domanda e report li scrive un agente: `untrusted`.

use super::super::turn::TOOL_RESULT_MAX;
use super::super::workspace_names::WorkspaceNames;
use super::{timestamp, without_nulls};
use crate::board::TaskView;
use crate::validate::clip_with_ellipsis;
use serde_json::{json, Map, Value};

/// Caratteri di testo concessi al task, dal piu' generoso al piu' stretto: si
/// usa il primo con cui l'esito (con le virgolette e gli a capo che il JSON
/// raddoppia) sta nel limite; con 0 resta solo la scheda del task.
const TEXT_BUDGETS: [usize; 4] = [4000, 2000, 1000, 0];
const SPEC_CAP: usize = 2500;
const SUMMARY_CAP: usize = 1500;
const NOTE_CAP: usize = 500;
const QUESTION_CAP: usize = 600;
const DIFFSTAT_CAP: usize = 600;
/// Caratteri di una voce di elenco (esito, follow-up, comando di test, file).
const ITEM_CAP: usize = 300;
/// Voci di un elenco di id (dipendenze, dipendenti).
const IDS_MAX: usize = 20;
/// Campi interni del task, che il segretario non deve vedere.
const INTERNAL_FIELDS: [&str; 5] = ["session", "last_report", "questions", "previous_attempt", "can_reorder"];
const TIME_FIELDS: [&str; 4] = ["created_at", "updated_at", "started_at", "finished_at"];

/// Il task `v` del workspace `workspace` per il modello.
pub fn task_details(v: &TaskView, workspace: &str, names: &WorkspaceNames) -> Value {
    let (last, tries) = TEXT_BUDGETS.split_last().unwrap_or((&0, &[]));
    for budget in tries {
        let out = build(v, workspace, names, *budget);
        if out.to_string().chars().count() <= TOOL_RESULT_MAX {
            return out;
        }
    }
    build(v, workspace, names, *last)
}

fn build(v: &TaskView, workspace: &str, names: &WorkspaceNames, budget: usize) -> Value {
    let Value::Object(mut task) = without_nulls(v.to_json(true)) else {
        return json!({ "error": "The task could not be read." });
    };
    for field in INTERNAL_FIELDS {
        task.remove(field);
    }
    for field in TIME_FIELDS {
        if let Some(ms) = task.get(field).and_then(Value::as_i64) {
            task.insert(field.to_string(), timestamp(ms));
        }
    }
    cap_ids(&mut task, "depends_on");
    cap_ids(&mut task, "waiting_on");
    if let Some(Value::Object(cascade)) = task.get_mut("cascade") {
        cap_ids(cascade, "dependents");
        cap_ids(cascade, "running");
    }
    shorten_worktree(&mut task);
    shorten_e2e(&mut task, "e2e_run");
    let mut text = TextBudget::new(budget);
    clip_question(&mut task, &mut text);
    clip_string(&mut task, "note", NOTE_CAP, &mut text);
    if let Some(Value::Object(report)) = task.get_mut("report") {
        shorten_e2e(report, "e2e");
        clip_string(report, "summary", SUMMARY_CAP, &mut text);
        clip_strings(report, "findings", &mut text);
        clip_strings(report, "follow_ups", &mut text);
        clip_tests(report, &mut text);
    }
    clip_string(&mut task, "task", SPEC_CAP, &mut text);
    if let Some(Value::Object(report)) = task.get_mut("report") {
        clip_string(report, "diffstat", DIFFSTAT_CAP, &mut text);
        clip_strings(report, "files_changed", &mut text);
    }
    let mut out = Value::Object(task);
    out["workspace"] = Value::from(workspace);
    if let Some(name) = names.name_of(workspace) {
        out["workspace_name"] = Value::from(name);
    }
    out["untrusted"] = Value::Bool(true);
    if !text.truncated.is_empty() {
        out["truncated"] = json!(text.truncated);
    }
    out
}

/// I caratteri di testo ancora disponibili e i campi che ne hanno perso un pezzo.
struct TextBudget {
    left: usize,
    truncated: Vec<&'static str>,
}

impl TextBudget {
    fn new(budget: usize) -> Self {
        Self { left: budget, truncated: Vec::new() }
    }

    /// `text` tagliato a `cap` caratteri e a quanto resta del budget.
    fn take(&mut self, field: &'static str, text: &str, cap: usize) -> String {
        let allowed = cap.min(self.left);
        let used = text.chars().count().min(allowed);
        if used < text.chars().count() {
            self.mark(field);
        }
        self.left -= used;
        clip_with_ellipsis(text, allowed)
    }

    fn mark(&mut self, field: &'static str) {
        if !self.truncated.contains(&field) {
            self.truncated.push(field);
        }
    }
}

fn clip_string(object: &mut Map<String, Value>, field: &'static str, cap: usize, text: &mut TextBudget) {
    if let Some(Value::String(value)) = object.get_mut(field) {
        *value = text.take(field, value, cap);
    }
}

/// Un elenco di stringhe: ogni voce tagliata, e le voci che non entrano nel budget tolte.
fn clip_strings(object: &mut Map<String, Value>, field: &'static str, text: &mut TextBudget) {
    let Some(Value::Array(items)) = object.get_mut(field) else {
        return;
    };
    let kept = items.iter().filter_map(Value::as_str).map_while(|item| keep(item, field, text)).collect::<Vec<_>>();
    if kept.len() < items.len() {
        text.mark(field);
    }
    *items = kept.into_iter().map(Value::from).collect();
}

fn keep(item: &str, field: &'static str, text: &mut TextBudget) -> Option<String> {
    (text.left > 0).then(|| text.take(field, item, ITEM_CAP))
}

/// Le prove eseguite: comando e dettagli tagliati, le prove che non entrano tolte.
fn clip_tests(report: &mut Map<String, Value>, text: &mut TextBudget) {
    const FIELD: &str = "tests";
    let Some(Value::Array(runs)) = report.get_mut(FIELD) else {
        return;
    };
    let total = runs.len();
    let mut kept = Vec::new();
    for run in runs.iter_mut() {
        if text.left == 0 {
            break;
        }
        if let Some(fields) = run.as_object_mut() {
            clip_string(fields, "command", ITEM_CAP, text);
            clip_string(fields, "details", ITEM_CAP, text);
        }
        kept.push(run.take());
    }
    if kept.len() < total {
        text.mark(FIELD);
    }
    *runs = kept;
}

/// La domanda del worker in attesa: id (per `relay_answer`), testo (tagliato)
/// e risposte suggerite.
fn clip_question(task: &mut Map<String, Value>, text: &mut TextBudget) {
    let Some(Value::Object(question)) = task.get_mut("question") else {
        return;
    };
    question.retain(|key, _| matches!(key.as_str(), "id" | "text" | "options"));
    clip_string(question, "text", QUESTION_CAP, text);
    clip_strings(question, "options", text);
}

/// Al massimo `IDS_MAX` id di un elenco.
fn cap_ids(object: &mut Map<String, Value>, field: &str) {
    if let Some(Value::Array(ids)) = object.get_mut(field) {
        ids.truncate(IDS_MAX);
    }
}

/// Del worktree solo branch e base: il percorso e' della macchina dell'utente.
fn shorten_worktree(task: &mut Map<String, Value>) {
    if let Some(Value::Object(worktree)) = task.get_mut("worktree") {
        worktree.retain(|key, _| key == "branch" || key == "base");
    }
}

/// Di un'esecuzione E2E solo l'esito.
fn shorten_e2e(object: &mut Map<String, Value>, field: &str) {
    if let Some(run) = object.get_mut(field) {
        *run = json!({ "status": run["status"] });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{
        build_report, AgentGate, Board, Finished, ReportFields, TaskQuestion, TaskSpec, TaskState, TestRun, Verdict,
    };
    use crate::hub::engine::fixtures::names;

    fn board(configure: impl FnOnce(&mut Board)) -> TaskView {
        let mut board = Board::new("ws-1");
        let spec = TaskSpec {
            id: "t-1".into(),
            title: "Login".into(),
            task: "Build the sign-in page".into(),
            ..TaskSpec::default()
        };
        board.create_task(spec, None, 1_700_000_000_000).unwrap();
        configure(&mut board);
        board.view_with(None, false, &AgentGate::default()).tasks.remove(0)
    }

    fn finished(report: crate::board::TaskReport) -> TaskState {
        TaskState::Done(Finished {
            started_at: Some(1_700_000_001_000),
            finished_at: 1_700_000_002_000,
            report: Some(report),
        })
    }

    fn long_report(items: usize) -> crate::board::TaskReport {
        let fields = ReportFields {
            commit: Some("abc1234".into()),
            pr_url: Some("https://example.com/pr/7".into()),
            files_changed: (0..items).map(|i| format!("src/file_{i}.rs")).collect(),
            tests: (0..items)
                .map(|i| TestRun {
                    command: format!("cargo test {i}"),
                    result: "passed".into(),
                    details: Some("ok".into()),
                })
                .collect(),
            follow_ups: (0..items).map(|i| format!("Follow up {i}: {}", "x".repeat(200))).collect(),
            verdict: Some(Verdict::Issues),
            findings: vec!["Missing test".into()],
        };
        build_report("completed", &"Summary. ".repeat(400), fields, Some(" 3 files changed".into()))
    }

    fn details(v: &TaskView) -> Value {
        task_details(v, "ws-1", &names())
    }

    #[test]
    fn a_done_task_carries_its_full_report_and_no_internal_field() {
        let v = board(|b| b.task_mut("t-1").unwrap().set_state(finished(long_report(2))));
        let out = details(&v);
        assert_eq!(
            (out["id"].as_str(), out["status"].as_str(), out["workspace_name"].as_str()),
            (Some("t-1"), Some("done"), Some("Muse Companion"))
        );
        assert_eq!(out["task"], "Build the sign-in page");
        let report = &out["report"];
        assert_eq!(
            (report["commit"].as_str(), report["pr_url"].as_str()),
            (Some("abc1234"), Some("https://example.com/pr/7"))
        );
        assert_eq!(report["files_changed"], json!(["src/file_0.rs", "src/file_1.rs"]));
        assert_eq!(report["tests"][0], json!({"command": "cargo test 0", "result": "passed", "details": "ok"}));
        assert_eq!(
            (report["verdict"].as_str(), report["findings"][0].as_str()),
            (Some("issues"), Some("Missing test"))
        );
        assert_eq!(report["diffstat"], " 3 files changed");
        assert_eq!(out["created_at"], "2023-11-14T22:13:20Z");
        assert_eq!(out["finished_at"], "2023-11-14T22:13:22Z");
        for hidden in ["session", "last_report", "questions", "previous_attempt", "can_reorder"] {
            assert!(out.get(hidden).is_none(), "{hidden}");
        }
        assert!(out["allowed_actions"].is_array() && out["can_edit"].is_boolean() && out["cascade"].is_object());
    }

    #[test]
    fn what_an_agent_wrote_is_marked_untrusted() {
        assert_eq!(details(&board(|_| {}))["untrusted"], true);
    }

    #[test]
    fn the_pending_question_shows_its_id_text_and_options_only() {
        let mut v = board(|_| {});
        v.question = Some(TaskQuestion {
            id: "q-1-1".into(),
            attempt: 1,
            text: "Which database?".into(),
            options: vec!["Postgres".into(), "SQLite".into()],
            asked_at: 0,
            answered_at: None,
            answer: None,
        });
        let out = details(&v);
        assert_eq!(
            out["question"],
            json!({"id": "q-1-1", "text": "Which database?", "options": ["Postgres", "SQLite"]})
        );
    }

    #[test]
    fn the_worktree_shows_the_branch_but_not_the_path() {
        let v = board(|b| {
            b.task_mut("t-1").unwrap().worktree = Some(crate::board::WorktreeInfo {
                path: "C:/secret/place".into(),
                branch: "task/login".into(),
                base: "origin/main".into(),
                setup_done: true,
            });
        });
        let out = details(&v);
        assert_eq!(out["worktree"], json!({"branch": "task/login", "base": "origin/main"}));
        assert!(!out.to_string().contains("secret"));
    }

    #[test]
    fn a_huge_report_is_cut_to_fit_and_says_what_was_cut() {
        let v = board(|b| b.task_mut("t-1").unwrap().set_state(finished(long_report(80))));
        let out = details(&v);
        assert!(out.to_string().chars().count() <= TOOL_RESULT_MAX, "{}", out.to_string().chars().count());
        let cut: Vec<&str> = out["truncated"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(cut.contains(&"summary") && cut.contains(&"follow_ups") && cut.contains(&"files_changed"), "{cut:?}");
        assert!(out["report"]["summary"].as_str().unwrap().ends_with('…'));
        assert_eq!(out["report"]["commit"], "abc1234", "the short facts survive");
    }

    #[test]
    fn quotes_and_newlines_that_json_doubles_still_fit() {
        let nasty = "\"\n".repeat(5000);
        let v = board(|b| {
            let t = b.task_mut("t-1").unwrap();
            t.task = nasty.clone();
            t.set_note(Some(&nasty));
            t.set_state(finished(build_report("completed", &nasty, ReportFields::default(), None)));
        });
        let out = details(&v);
        assert!(out.to_string().chars().count() <= TOOL_RESULT_MAX);
        assert!(out["truncated"].as_array().is_some_and(|l| l.len() >= 2));
    }

    #[test]
    fn a_short_task_is_not_marked_truncated() {
        assert!(details(&board(|_| {})).get("truncated").is_none());
    }

    #[test]
    fn id_lists_are_capped() {
        let v = board(|b| {
            b.task_mut("t-1").unwrap().depends_on = (0..60).map(|i| format!("dep-{i}")).collect();
        });
        assert_eq!(details(&v)["depends_on"].as_array().map(Vec::len), Some(IDS_MAX));
    }
}
