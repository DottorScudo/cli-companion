//! Il Bot di un task: obbligatorio in `propose_tasks`, verificato alla proposta e
//! all'accettazione, copiato nel task (agente, modello, effort, skill) e non piu'
//! legato al Bot dopo. Le proposte vecchie, senza Bot, valgono come prima.

use super::*;
use crate::agents::{registry, Effort};
use crate::bots::AgentBot;

const NO_BOT: &str = "tasks[0]: task a: bot is required (see get_workspace_settings.bots)";

fn real_id() -> &'static str {
    registry::default_agent().id()
}

/// Il modello della fascia forte dell'agente reale: quello del Bot `intake`.
fn strong_model() -> String {
    crate::agents::tier_models(registry::default_agent()).unwrap().strong.to_string()
}

/// Host con la board in pausa: i task restano in coda.
fn paused() -> (AutomationState, FakeHost) {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    (st, h)
}

fn with_bot(id: &str, bot: &str) -> Value {
    json!({"id": id, "title": "T", "task": "x", "bot": bot})
}

fn propose(st: &AutomationState, h: &FakeHost, tasks: Vec<Value>) -> Result<String, String> {
    let out = call(st, h, INTAKE, "propose_tasks", json!({"tasks": tasks}))?;
    Ok(out["proposal"].as_str().unwrap().to_string())
}

fn accept(st: &AutomationState, h: &FakeHost, proposal: &str) -> Result<BoardView, String> {
    boards::proposal_decide(st, h, "ws-1", proposal, true)
}

fn edit_bot(h: &FakeHost, id: &str, f: impl FnOnce(&mut AgentBot)) {
    h.edit_bots(|bots| f(bots.iter_mut().find(|b| b.id == id).unwrap()));
}

#[test]
fn a_proposal_task_without_a_bot_is_rejected() {
    let (st, h) = paused();
    let no_bot = json!({"id": "a", "title": "T", "task": "x", "model": "sonnet"});
    assert_eq!(propose(&st, &h, vec![no_bot]), Err(NO_BOT.to_string()));
    let null_bot = json!({"id": "a", "title": "T", "task": "x", "bot": null});
    assert_eq!(propose(&st, &h, vec![null_bot]), Err(NO_BOT.to_string()));
    assert!(view(&st, &h).proposals.is_empty());
}

#[test]
fn an_unknown_bot_is_rejected_with_the_way_to_list_them() {
    let (st, h) = paused();
    let err = propose(&st, &h, vec![with_bot("a", "nope")]).unwrap_err();
    assert_eq!(err, "task 'a': unknown bot: nope (see get_workspace_settings.bots)");
    assert!(view(&st, &h).proposals.is_empty());
}

#[test]
fn a_disabled_bot_is_rejected_at_the_proposal_and_at_the_acceptance() {
    let (st, h) = paused();
    edit_bot(&h, "intake", |b| b.enabled = false);
    let err = propose(&st, &h, vec![with_bot("a", "intake")]).unwrap_err();
    assert_eq!(err, "task 'a': Intake is disabled: enable it in the bots settings");

    // Disabilitato dopo la proposta: l'accettazione lo ricontrolla e la proposta resta.
    edit_bot(&h, "intake", |b| b.enabled = true);
    let proposal = propose(&st, &h, vec![with_bot("a", "intake")]).unwrap();
    edit_bot(&h, "intake", |b| b.enabled = false);
    let err = accept(&st, &h, &proposal).unwrap_err();
    assert_eq!(err, "task 'a': Intake is disabled: enable it in the bots settings");
    assert_eq!((view(&st, &h).proposals.len(), view(&st, &h).tasks.len()), (1, 0));
}

#[test]
fn an_agent_that_is_not_the_one_of_the_bot_is_rejected() {
    let (st, h) = paused();
    let mut other = with_bot("a", "general");
    other["agent"] = json!("opencode");
    let err = propose(&st, &h, vec![other]).unwrap_err();
    let expected = format!(
        "task 'a': agent opencode does not match the agent of bot general ({}): omit agent or choose another bot",
        real_id()
    );
    assert_eq!(err, expected);
}

#[test]
fn acceptance_copies_the_agent_model_effort_and_skill_of_the_bot_into_the_task() {
    let (st, h) = paused();
    let proposal = propose(&st, &h, vec![with_bot("a", "intake")]).unwrap();
    // La proposta tiene la specifica com'e': niente copia finche' non si accetta.
    let written = &view(&st, &h).proposals[0].tasks[0];
    assert_eq!((written.bot.as_deref(), written.model.as_deref(), written.effort), (Some("intake"), None, None));

    accept(&st, &h, &proposal).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!((t["bot"].as_str(), t["agent"].as_str()), (Some("intake"), Some(real_id())));
    assert_eq!(t["model"], json!(strong_model()));
    assert_eq!(t["effort"], "high");
    assert_eq!(t["skill"], json!({"kind": "plugin", "id": "task-intake"}));
}

#[test]
fn a_task_of_the_general_bot_has_no_effort_and_no_skill() {
    let (st, h) = paused();
    let proposal = propose(&st, &h, vec![with_bot("a", "general")]).unwrap();
    accept(&st, &h, &proposal).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!((t["bot"].as_str(), t["agent"].as_str()), (Some("general"), Some(real_id())));
    for absent in ["effort", "skill"] {
        assert!(t.get(absent).is_none(), "{absent}");
    }
    assert!(t["model"].is_null(), "the CLI default decides");
}

#[test]
fn the_model_of_the_proposal_wins_over_the_model_of_the_bot() {
    let (st, h) = paused();
    let mut task_spec = with_bot("a", "intake");
    task_spec["model"] = json!("sonnet");
    let proposal = propose(&st, &h, vec![task_spec]).unwrap();
    accept(&st, &h, &proposal).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!(t["model"], "sonnet");
    assert_eq!(t["effort"], "high", "the rest still comes from the bot");
}

#[test]
fn editing_or_deleting_the_bot_does_not_change_a_task_already_queued() {
    let (st, h) = paused();
    let proposal = propose(&st, &h, vec![with_bot("a", "intake")]).unwrap();
    accept(&st, &h, &proposal).unwrap();
    let before = task(&st, &h, "a");

    edit_bot(&h, "intake", |b| {
        b.effort = Some(Effort::Low);
        b.skill = None;
        b.model = crate::bots::default_agent_bots().remove(0).model;
    });
    assert_eq!(task(&st, &h, "a"), before);
    h.edit_bots(|bots| bots.retain(|b| b.id != "intake"));
    assert_eq!(task(&st, &h, "a"), before);

    // Una modifica che non tocca chi esegue il task non rilegge il Bot.
    call(&st, &h, INTAKE, "update_task", json!({"id": "a", "title": "Renamed"})).unwrap();
    let after = task(&st, &h, "a");
    assert_eq!((after["title"].as_str(), after["bot"].as_str()), (Some("Renamed"), Some("intake")));
    for kept in ["model", "effort", "skill"] {
        assert_eq!(after[kept], before[kept], "{kept}");
    }
}

#[test]
fn changing_the_bot_of_a_task_takes_a_new_copy_and_null_removes_it() {
    let (st, h) = paused();
    let proposal = propose(&st, &h, vec![with_bot("a", "intake")]).unwrap();
    accept(&st, &h, &proposal).unwrap();

    let err = call(&st, &h, INTAKE, "update_task", json!({"id": "a", "bot": "nope"})).unwrap_err();
    assert_eq!(err, "task 'a': unknown bot: nope (see get_workspace_settings.bots)");

    call(&st, &h, INTAKE, "update_task", json!({"id": "a", "bot": "setup"})).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!((t["bot"].as_str(), t["effort"].as_str()), (Some("setup"), Some("medium")));
    assert_eq!(t["skill"], json!({"kind": "plugin", "id": "project-setup"}));
    assert_ne!(t["model"], json!(strong_model()), "the model of the new bot, not the old copy");

    call(&st, &h, INTAKE, "update_task", json!({"id": "a", "bot": null})).unwrap();
    let t = task(&st, &h, "a");
    assert!(t.get("bot").is_none() && t.get("effort").is_none() && t.get("skill").is_none(), "{t}");
}

#[test]
fn a_pending_proposal_saved_before_the_bots_is_accepted_as_before() {
    let (st, h) = paused();
    let proposal = propose(&st, &h, vec![with_bot("a", "general")]).unwrap();
    // Come una proposta salvata da una versione precedente: senza `bot`.
    let old = json!({"id": "a", "title": "T", "task": "x", "model": "sonnet"});
    st.run(&h, |r| r.boards.get_mut("ws-1").unwrap().proposals[0].tasks[0] = serde_json::from_value(old).unwrap());
    accept(&st, &h, &proposal).unwrap();
    let t = task(&st, &h, "a");
    assert_eq!((t["agent"].as_str(), t["model"].as_str()), (Some(real_id()), Some("sonnet")));
    for absent in ["bot", "effort", "skill"] {
        assert!(t.get(absent).is_none(), "{absent}");
    }
}

#[test]
fn tasks_created_from_the_ui_do_not_need_a_bot() {
    let (st, h) = paused();
    boards::task_create(&st, &h, "ws-1", &json!({"title": "Manual", "task": "x"})).unwrap();
    assert!(task(&st, &h, "manual").get("bot").is_none());
    boards::task_create(&st, &h, "ws-1", &json!({"title": "With bot", "task": "x", "bot": "intake"})).unwrap();
    assert_eq!(task(&st, &h, "with-bot")["effort"], "high");
    let err = boards::task_create(&st, &h, "ws-1", &json!({"title": "Bad", "task": "x", "bot": "nope"})).unwrap_err();
    assert!(err.contains("unknown bot: nope"), "{err}");
}
