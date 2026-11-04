use super::*;
use serde_json::json;

/// Un file scritto dall'app di oggi (senza busta), preso da una board vera.
const V0: &str = include_str!("../fixtures/board_v0.json");
/// Un file della v0.11, con stati e campi che non esistono piu'.
const V011: &str = include_str!("../fixtures/board_v011.json");

fn migrated(raw: &str) -> Value {
    v0_to_v1(serde_json::from_str(raw).unwrap()).unwrap()
}

fn task<'a>(board: &'a Value, id: &str) -> &'a Value {
    board["tasks"].as_array().unwrap().iter().find(|t| t["id"] == id).unwrap()
}

#[test]
fn the_format_written_today_passes_through_untouched() {
    let original: Value = serde_json::from_str(V0).unwrap();
    // Anche il task `running`: lo conclude il recupero, non la migrazione.
    assert_eq!(task(&original, "task-1")["status"], "running");
    assert_eq!(migrated(V0), original);
}

#[test]
fn v011_statuses_are_mapped_and_cancelled_tasks_removed() {
    let v = migrated(V011);
    let ids: Vec<&str> = v["tasks"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["queued", "running", "review", "blocked", "failed", "done"]);
    let status = |id: &str| task(&v, id)["status"].as_str().unwrap().to_string();
    assert_eq!(
        ["queued", "running", "review", "blocked", "failed", "done"].map(status),
        ["queued", "running", "failed", "failed", "failed", "done"]
    );
}

#[test]
fn a_review_task_gets_the_unintegrated_note_and_keeps_its_report() {
    let v = migrated(V011);
    let review = task(&v, "review");
    assert_eq!(review["note"], UNINTEGRATED_NOTE);
    assert_eq!(review["report"], json!({"status": "completed", "summary": "done work"}));
    // Senza l'ora la migrazione non inventa date: le fissa la lettura del record.
    assert!(review.get("finished_at").is_none() && review.get("updated_at").is_none());
}

#[test]
fn a_blocked_task_and_its_report_become_failed() {
    let v = migrated(V011);
    let blocked = task(&v, "blocked");
    assert_eq!(blocked["note"], "stuck");
    assert_eq!(blocked["report"], json!({"status": "failed", "summary": "cannot"}));
}

#[test]
fn dependencies_on_cancelled_tasks_are_dropped() {
    let v = migrated(V011);
    assert_eq!(task(&v, "done")["depends_on"], json!(["queued"]));
    assert_eq!(task(&v, "running")["depends_on"], json!(["queued"]));
}

#[test]
fn worker_tabs_survive_only_on_running_tasks() {
    let v = migrated(V011);
    let running = task(&v, "running");
    assert_eq!((&running["session"], &running["worker_status"]), (&json!("auto-1"), &json!("running")));
    for id in ["queued", "review", "blocked", "failed", "done"] {
        let t = task(&v, id);
        assert_eq!((&t["session"], &t["worker_status"]), (&Value::Null, &Value::Null), "{id}");
    }
}

#[test]
fn a_file_that_is_not_a_board_passes_through() {
    for other in [json!({"x": 1}), json!([1, 2]), json!({"tasks": "no"}), json!(null)] {
        assert_eq!(v0_to_v1(other.clone()).unwrap(), other);
    }
}

#[test]
fn migrating_twice_changes_nothing() {
    let once = migrated(V011);
    assert_eq!(v0_to_v1(once.clone()).unwrap(), once);
}
