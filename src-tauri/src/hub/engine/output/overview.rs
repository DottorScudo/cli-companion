//! Il riepilogo di tutti i workspace leggibili, per `board_overview` (puro):
//! per ognuno nome, conteggi per stato (l'intestazione di `board_status`), i
//! task che aspettano l'utente e le domande dei worker senza risposta, entrambi
//! in poche righe, con il totale quando ce ne sono di piu'.

use super::super::board_query::{self, BoardQuery};
use super::super::workspace_names::WorkspaceNames;
use crate::board::{BoardView, TaskView};
use crate::hub::settings::Secretary;
use crate::validate::clip_with_ellipsis;
use serde_json::{json, Value};

/// Workspace riportati al massimo: oltre, `omitted_workspaces` ne conta il resto.
const WORKSPACES_MAX: usize = 20;
/// Righe di task con attenzione e di domande per workspace.
const ROWS_MAX: usize = 3;
/// Caratteri tenuti del testo di una domanda.
const QUESTION_KEPT: usize = 120;

/// Il riepilogo dei workspace che `secretary` puo' consultare, nell'ordine dello
/// store, con le `views` correnti delle board.
pub fn board_overview(views: &[BoardView], names: &WorkspaceNames, secretary: &Secretary) -> Value {
    let mut entries = names
        .readable_by(secretary)
        .filter_map(|(id, name)| views.iter().find(|v| v.workspace_id == id).map(|view| workspace(view, name)));
    let workspaces: Vec<Value> = entries.by_ref().take(WORKSPACES_MAX).collect();
    let mut out = json!({ "workspaces": workspaces });
    let omitted = entries.count();
    if omitted > 0 {
        out["omitted_workspaces"] = Value::from(omitted);
    }
    out
}

fn workspace(view: &BoardView, name: &str) -> Value {
    let mut out = view.summary_json();
    out["workspace"] = Value::from(view.workspace_id.as_str());
    out["workspace_name"] = Value::from(name);
    let query = BoardQuery { attention: true, limit: ROWS_MAX, ..BoardQuery::default() };
    let attention = board_query::select(&view.tasks, &query);
    if attention.total > 0 {
        out["attention_total"] = Value::from(attention.total);
        out["attention_tasks"] = attention.tasks.into_iter().map(attention_row).collect();
    }
    let questions: Vec<&TaskView> = view.tasks.iter().filter(|v| v.question.is_some()).collect();
    if !questions.is_empty() {
        out["questions_total"] = Value::from(questions.len());
        out["questions"] = questions.into_iter().take(ROWS_MAX).filter_map(question_row).collect();
    }
    out
}

fn attention_row(v: &TaskView) -> Value {
    json!({ "id": v.task.id, "title": v.task.title, "status": v.task.status().as_str() })
}

fn question_row(v: &TaskView) -> Option<Value> {
    let question = v.question.as_ref()?;
    Some(json!({ "task_id": v.task.id, "text": clip_with_ellipsis(&question.text, QUESTION_KEPT) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{AgentGate, Board, TaskQuestion, TaskSpec, TaskStatus};
    use crate::hub::channel::Channel;
    use crate::hub::engine::fixtures::{names, secretary};
    use crate::hub::settings::SecretaryChannels;

    fn board(id: &str, tasks: usize, paused: bool) -> BoardView {
        let mut board = Board::new(id);
        for i in 0..tasks {
            let spec = TaskSpec { id: format!("{id}-t{i}"), title: format!("Task {i}"), ..TaskSpec::default() };
            board.create_task(spec, None, 1_700_000_000_000 + i as i64).unwrap();
        }
        board.paused = paused;
        board.view_with(None, false, &AgentGate::default())
    }

    fn ask(view: &mut BoardView, index: usize, text: &str) {
        view.tasks[index].task.set_status(TaskStatus::Running);
        view.tasks[index].question = Some(TaskQuestion {
            id: "q-1-1".into(),
            attempt: 1,
            text: text.into(),
            options: Vec::new(),
            asked_at: 0,
            answered_at: None,
            answer: None,
        });
    }

    #[test]
    fn every_readable_workspace_has_its_name_counts_and_pause() {
        let views = [board("ws-1", 2, false), board("ws-2", 0, true)];
        let out = board_overview(&views, &names(), &secretary("mario"));
        let list = out["workspaces"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0]["workspace"].as_str(), list[0]["workspace_name"].as_str()),
            (Some("ws-1"), Some("Muse Companion"))
        );
        assert_eq!(list[0]["counts"], json!({"queued": 2, "running": 0, "done": 0, "failed": 0}));
        assert!(list[0].get("paused").is_none() && list[0].get("attention_tasks").is_none());
        assert_eq!((list[1]["workspace_name"].as_str(), list[1]["paused"].as_bool()), (Some("Sito Web"), Some(true)));
        assert!(out.get("omitted_workspaces").is_none());
    }

    #[test]
    fn only_the_workspaces_of_the_secretary_channels_are_listed() {
        let views = [board("ws-1", 1, false), board("ws-2", 1, false)];
        let mut s = secretary("mario");
        s.channels = SecretaryChannels::Only { channels: vec![Channel::Workspace("ws-2".into())] };
        let out = board_overview(&views, &names(), &s);
        let list = out["workspaces"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["workspace"], "ws-2");
        assert!(!out.to_string().contains("Muse Companion"));
        s.channels = SecretaryChannels::Only { channels: Vec::new() };
        assert_eq!(board_overview(&views, &names(), &s), json!({"workspaces": []}));
    }

    #[test]
    fn tasks_that_need_the_user_and_open_questions_are_listed_with_their_totals() {
        let mut view = board("ws-1", 6, false);
        for i in 0..5 {
            ask(&mut view, i, &format!("Question {i} {}", "x".repeat(300)));
        }
        let out = board_overview(&[view], &names(), &secretary("mario"));
        let ws = &out["workspaces"][0];
        assert_eq!((ws["attention_total"].as_u64(), ws["questions_total"].as_u64()), (Some(5), Some(5)));
        assert_eq!(ws["attention_tasks"].as_array().map(Vec::len), Some(ROWS_MAX));
        assert_eq!(ws["attention_tasks"][0], json!({"id": "ws-1-t4", "title": "Task 4", "status": "running"}));
        let questions = ws["questions"].as_array().unwrap();
        assert_eq!(questions.len(), ROWS_MAX);
        assert_eq!(questions[0]["task_id"], "ws-1-t0");
        assert_eq!(questions[0]["text"].as_str().map(|t| t.chars().count()), Some(QUESTION_KEPT + 1));
    }

    #[test]
    fn a_workspace_without_a_board_view_is_skipped_and_the_list_is_capped() {
        let ids: Vec<String> = (0..WORKSPACES_MAX + 3).map(|i| format!("w-{i}")).collect();
        let many = crate::hub::engine::workspace_names::WorkspaceNames::new(
            ids.iter().map(|id| (id.clone(), format!("Name {id}"))).chain([("gone".to_string(), "Gone".to_string())]),
        );
        let views: Vec<BoardView> = ids.iter().map(|id| board(id, 0, false)).collect();
        let out = board_overview(&views, &many, &secretary("mario"));
        assert_eq!(out["workspaces"].as_array().map(Vec::len), Some(WORKSPACES_MAX));
        assert_eq!(out["omitted_workspaces"], 3);
        assert!(!out.to_string().contains("Gone"));
    }
}
