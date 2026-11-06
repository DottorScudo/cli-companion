//! I tipi di task dei Bot predefiniti veri: un Bot esegue solo i tipi dei suoi
//! `kinds`, alla proposta, alla creazione del piano dell'HUB e alla modifica di
//! un task (anche la verifica di `request_task_update`, prima della card).

use super::*;

/// Host con i Bot predefiniti veri e la board in pausa: i task restano in coda.
fn with_presets() -> (AutomationState, FakeHost) {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    h.edit_bots(|bots| *bots = crate::bots::default_agent_bots());
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    (st, h)
}

fn propose(st: &AutomationState, h: &FakeHost, tasks: Vec<Value>) -> Result<Value, String> {
    call(st, h, INTAKE, "propose_tasks", json!({"tasks": tasks}))
}

fn task_of(id: &str, kind: &str, bot: &str, deps: &[&str]) -> Value {
    json!({"id": id, "title": "T", "task": "x", "kind": kind, "bot": bot, "depends_on": deps})
}

/// Mette in coda il task `a` (Bot General) da cui dipendono le revisioni.
fn queue_a(st: &AutomationState, h: &FakeHost) {
    let r = propose(st, h, vec![task_of("a", "work", "general", &[])]).unwrap();
    boards::proposal_decide(st, h, "ws-1", r["proposal"].as_str().unwrap(), true).unwrap();
}

#[test]
fn a_review_proposed_with_general_is_refused_and_with_reviewer_passes() {
    let (st, h) = with_presets();
    queue_a(&st, &h);
    let err = propose(&st, &h, vec![task_of("rev", "review", "general", &["a"])]).unwrap_err();
    assert_eq!(
        err,
        "task 'rev': bot General cannot run review tasks: choose a bot whose kinds include review (see \
         get_workspace_settings.bots)"
    );
    assert!(propose(&st, &h, vec![task_of("rev", "review", "reviewer", &["a"])]).is_ok());
}

#[test]
fn a_work_task_proposed_with_an_interactive_bot_is_refused() {
    let (st, h) = with_presets();
    for bot in ["intake", "setup", "skill-wizard", "reviewer", "planner"] {
        let err = propose(&st, &h, vec![task_of("w", "work", bot, &[])]).unwrap_err();
        assert!(err.contains("cannot run work tasks"), "{bot}: {err}");
    }
    assert!(view(&st, &h).proposals.is_empty());
}

#[test]
fn a_plan_of_the_hub_needs_a_bot_that_plans() {
    let (st, h) = with_presets();
    let err = planner::create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "general").unwrap_err();
    assert!(err.contains("bot General cannot run plan tasks"), "{err}");
    assert!(view(&st, &h).tasks.is_empty());
    let id = planner::create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "planner").unwrap();
    assert_eq!(
        (task(&st, &h, &id)["kind"].as_str(), task(&st, &h, &id)["bot"].as_str()),
        (Some("plan"), Some("planner"))
    );
}

#[test]
fn a_bot_change_that_cannot_run_the_kind_of_the_task_is_refused_by_the_check() {
    let (st, h) = with_presets();
    let id = planner::create_hub_plan_task(&st, &h, "ws-1", "Plan", "Plan it", "planner").unwrap();
    let err = boards::task_update_check(&st, &h, "ws-1", &id, &json!({"bot": "general"})).unwrap_err();
    assert!(err.contains("bot General cannot run plan tasks"), "{err}");
    assert_eq!(task(&st, &h, &id)["bot"], "planner");
}

#[test]
fn changing_the_kind_of_a_queued_task_checks_its_bot() {
    let (st, h) = with_presets();
    queue_a(&st, &h);
    let r = propose(&st, &h, vec![task_of("b", "work", "general", &["a"])]).unwrap();
    boards::proposal_decide(&st, &h, "ws-1", r["proposal"].as_str().unwrap(), true).unwrap();
    let err = boards::task_update(&st, &h, "ws-1", "b", &json!({"kind": "review"})).unwrap_err();
    assert!(err.contains("bot General cannot run review tasks"), "{err}");
    assert_eq!(task(&st, &h, "b")["kind"], "work");
    boards::task_update(&st, &h, "ws-1", "b", &json!({"kind": "review", "bot": "reviewer"})).unwrap();
    assert_eq!(task(&st, &h, "b")["kind"], "review");
}
