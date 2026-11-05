//! Task in coda con l'agente non pronto nel target: non falliscono, aspettano
//! (con il motivo nella vista) e partono quando l'agente lo diventa. La
//! prontezza dell'host finto si imposta con `set_readiness`.

use super::*;
use crate::agents::registry;
use crate::agents::test_registry::SecondIntegration;
use crate::agents::testing::{FakeIntegration, FAKE_MODEL_SMALL};

const OTHER: &str = "other-agent";

fn default_id() -> &'static str {
    registry::default_agent().id()
}

/// Host con la consegna in place (un worker per repository).
fn host() -> (AutomationState, FakeHost) {
    (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false))
}

fn status(st: &AutomationState, h: &FakeHost, id: &str) -> String {
    task(st, h, id)["status"].as_str().unwrap().to_string()
}

/// Riga di `list_tasks` del task.
fn list_row(st: &AutomationState, h: &FakeHost, id: &str) -> Value {
    let list = call(st, h, INTAKE, "list_tasks", json!({})).unwrap();
    list["tasks"].as_array().unwrap().iter().find(|t| t["id"] == id).cloned().unwrap()
}

/// Specifica eseguita dal secondo agente del registro di test.
fn other_spec(id: &str) -> Value {
    let mut s = spec(id, &[]);
    s["agent"] = json!(OTHER);
    s["model"] = json!(FAKE_MODEL_SMALL);
    s
}

#[test]
fn a_task_whose_agent_is_not_ready_waits_with_the_reason_and_starts_when_it_becomes_ready() {
    let (st, h) = host();
    h.set_readiness(default_id(), Some(Readiness::NotFound));
    queue(&st, &h, vec![spec("a", &[])]);
    // Resta in coda, niente avvio e niente errore.
    assert_eq!(status(&st, &h, "a"), "queued");
    assert!(h.spawns().is_empty());
    assert!(h.git_log_without_detection().is_empty());
    // Il motivo e' nella vista della board, in list_tasks e in get_task_details.
    let expected = json!({"agent": default_id(), "reason": "not-found"});
    assert_eq!(task(&st, &h, "a")["agent_unavailable"], expected);
    assert_eq!(view(&st, &h).tasks[0].agent_unavailable.as_ref().map(|u| u.reason), Some(Readiness::NotFound));
    assert_eq!(list_row(&st, &h, "a")["agent_unavailable"], expected);
    let details = call(&st, &h, INTAKE, "get_task_details", json!({"id": "a"})).unwrap();
    assert_eq!(details["task"]["agent_unavailable"], expected);
    // Anche la vista emessa alla UI lo porta.
    assert_eq!(
        h.board_events.lock().unwrap().last().unwrap().tasks[0].agent_unavailable.as_ref().unwrap().agent,
        default_id()
    );

    // Finche' non cambia nulla, il giro non fa partire niente.
    st.tick(&h, now_millis());
    assert_eq!(status(&st, &h, "a"), "queued");

    // L'agente diventa pronto: il giro successivo avvia il task e il motivo sparisce.
    h.set_readiness(default_id(), Some(Readiness::Ready));
    st.tick(&h, now_millis());
    assert_eq!(status(&st, &h, "a"), "running");
    assert_eq!(h.spawns().len(), 1);
    assert!(task(&st, &h, "a").get("agent_unavailable").is_none());
}

#[test]
fn every_not_ready_state_is_reported_as_its_own_reason() {
    for (state, name) in [
        (Readiness::NotInstalled, "not-installed"),
        (Readiness::Outdated, "outdated"),
        (Readiness::Disabled, "disabled"),
    ] {
        let (st, h) = host();
        h.set_readiness(default_id(), Some(state));
        queue(&st, &h, vec![spec("a", &[])]);
        assert_eq!(status(&st, &h, "a"), "queued", "{name}");
        assert_eq!(task(&st, &h, "a")["agent_unavailable"]["reason"], name);
    }
}

#[test]
fn a_task_whose_agent_is_still_being_checked_waits_without_a_reason() {
    let (st, h) = host();
    h.set_readiness(default_id(), None);
    queue(&st, &h, vec![spec("a", &[])]);
    assert_eq!(status(&st, &h, "a"), "queued");
    assert!(h.spawns().is_empty());
    assert!(task(&st, &h, "a").get("agent_unavailable").is_none());
    assert!(list_row(&st, &h, "a").get("agent_unavailable").is_none());
    // Il controllo finisce: parte.
    h.set_readiness(default_id(), Some(Readiness::Ready));
    st.tick(&h, now_millis());
    assert_eq!(status(&st, &h, "a"), "running");
}

#[test]
fn a_blocked_task_does_not_hold_back_a_queued_task_of_a_ready_agent() {
    let _other = SecondIntegration::enabled(FakeIntegration::full(OTHER));
    let (st, h) = host();
    h.set_readiness(default_id(), Some(Readiness::NotFound));
    h.set_readiness(OTHER, Some(Readiness::NotFound));
    queue(&st, &h, vec![other_spec("a"), spec("b", &[])]);
    assert_eq!((status(&st, &h, "a"), status(&st, &h, "b")), ("queued".into(), "queued".into()));
    // Stesso repository, consegna in place: se "a" (prima in coda) tenesse il
    // posto, "b" non partirebbe.
    h.set_readiness(default_id(), Some(Readiness::Ready));
    st.recheck_agents(&h);
    assert_eq!((status(&st, &h, "a"), status(&st, &h, "b")), ("queued".into(), "running".into()));
    assert_eq!(task(&st, &h, "a")["agent_unavailable"]["agent"], OTHER);
    assert!(task(&st, &h, "b").get("agent_unavailable").is_none());
}

#[test]
fn rechecking_without_a_change_does_not_republish_the_board() {
    let (st, h) = host();
    h.set_readiness(default_id(), Some(Readiness::NotFound));
    queue(&st, &h, vec![spec("a", &[])]);
    // Il primo giro dopo l'avvio riscrive tutte le board: lo si fa prima di misurare.
    st.tick(&h, now_millis());
    let (events, rev) = (h.board_events.lock().unwrap().len(), view(&st, &h).rev);
    st.recheck_agents(&h);
    st.tick(&h, now_millis());
    st.tick(&h, now_millis());
    assert_eq!((h.board_events.lock().unwrap().len(), view(&st, &h).rev), (events, rev));
}

#[test]
fn a_change_of_readiness_reevaluates_the_board_at_once_without_waiting_for_the_timer() {
    let (st, h) = host();
    h.set_readiness(default_id(), Some(Readiness::NotFound));
    queue(&st, &h, vec![spec("a", &[])]);
    // Il motivo cambia: la vista si ripubblica con quello nuovo.
    h.set_readiness(default_id(), Some(Readiness::Outdated));
    st.recheck_agents(&h);
    assert_eq!(task(&st, &h, "a")["agent_unavailable"]["reason"], "outdated");
    assert_eq!(
        h.board_events.lock().unwrap().last().unwrap().tasks[0].agent_unavailable.as_ref().unwrap().reason,
        Readiness::Outdated
    );
    // L'agente torna pronto: il task parte subito.
    h.set_readiness(default_id(), Some(Readiness::Ready));
    st.recheck_agents(&h);
    assert_eq!(status(&st, &h, "a"), "running");
}

#[test]
fn removing_the_workspace_forgets_its_blocked_agents() {
    let (st, h) = host();
    h.set_readiness(default_id(), Some(Readiness::NotFound));
    queue(&st, &h, vec![spec("a", &[])]);
    assert!(st.registry().gates.contains_key("ws-1"));
    st.remove_board(&h, &h.ws_for("ws-1").unwrap());
    assert!(st.registry().gates.is_empty());
}

#[test]
fn get_workspace_settings_reports_the_readiness_of_each_enabled_agent() {
    let (st, h) = host();
    let agents = |h: &FakeHost| call(&st, h, INTAKE, "get_workspace_settings", json!({})).unwrap()["agents"].clone();
    assert_eq!(agents(&h)[0]["readiness"], "ready");
    h.set_readiness(default_id(), Some(Readiness::NotInstalled));
    let listed = agents(&h);
    assert_eq!(listed[0]["id"], default_id());
    assert_eq!(listed[0]["readiness"], "not-installed");
    assert_eq!(h.probed(), [default_id(), default_id()], "una sonda fresca per agente a ogni lettura");
}

/// "a" occupa l'unico posto del repository, "b" aspetta in coda con l'agente
/// ancora pronto nella cache; restituisce la sessione di "a".
fn queued_behind_a_running_task(st: &AutomationState, h: &FakeHost) -> String {
    queue(st, h, vec![spec("a", &[]), spec("b", &[])]);
    assert_eq!((status(st, h, "a"), status(st, h, "b")), ("running".into(), "queued".into()));
    session_of(st, h, "a")
}

#[test]
fn a_task_whose_agent_gets_disabled_while_queued_stays_queued_instead_of_failing() {
    let (st, h) = host();
    let session = queued_behind_a_running_task(&st, &h);
    let spawned = h.spawns().len();

    // L'integrazione viene disabilitata mentre il valore noto dice ancora "pronto".
    h.set_readiness(default_id(), Some(Readiness::Disabled));
    st.agents_changed(&h);
    finish(&st, &h, &session, "completed");

    // Lo slot si libera, ma "b" non parte: resta in coda con il motivo.
    st.tick(&h, now_millis());
    assert_eq!(status(&st, &h, "b"), "queued");
    assert_eq!(task(&st, &h, "b")["agent_unavailable"], json!({"agent": default_id(), "reason": "disabled"}));
    assert_eq!(h.spawns().len(), spawned, "nessuno spawn");
}

#[test]
fn re_enabling_the_agent_restarts_the_queued_task_without_waiting_for_the_cache() {
    let (st, h) = host();
    let session = queued_behind_a_running_task(&st, &h);
    h.set_readiness(default_id(), Some(Readiness::Disabled));
    st.agents_changed(&h);
    finish(&st, &h, &session, "completed");
    assert_eq!(status(&st, &h, "b"), "queued");

    h.set_readiness(default_id(), Some(Readiness::Ready));
    st.agents_changed(&h);
    assert_eq!(status(&st, &h, "b"), "running");
    assert!(task(&st, &h, "b").get("agent_unavailable").is_none());
}
