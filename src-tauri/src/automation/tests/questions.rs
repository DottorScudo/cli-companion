//! `ask_user`: il tool (ruoli, HUB, validazione), la macchina a stati del worker che
//! aspetta la risposta, la consegna della risposta nel terminale e le istruzioni.

use super::plan::started_plan;
use super::review::reviewer;
use super::*;
use crate::board::texts::{ASK_WITH_TOOL, PLANNER_ASK_USER};
use crate::board::{ANSWER_MAX, OPTIONS_MAX, OPTION_MAX, QUESTION_MAX};

/// Un worker che ha letto il task, con l'HUB acceso.
fn asker() -> (AutomationState, FakeHost, String) {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    h.set_hub(true);
    call(&st, &h, &w, "get_task", json!({})).unwrap();
    (st, h, w)
}

fn ask(st: &AutomationState, h: &FakeHost, w: &str, text: &str) -> Result<Value, String> {
    call(st, h, w, "ask_user", json!({ "question": text }))
}

fn answer(st: &AutomationState, h: &FakeHost, id: &str, text: &str) -> Result<(), String> {
    st.answer_question(h, "ws-1", "a", id, text)
}

fn status_of(st: &AutomationState, h: &FakeHost) -> Value {
    task(st, h, "a")["worker_status"].clone()
}

/// Il worker fa una domanda e chiude il turno: ora aspetta la risposta.
fn waiting() -> (AutomationState, FakeHost, String) {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "Which db?").unwrap();
    assert_eq!(stop(&st, &h, &w, Some(false), None), None);
    assert_eq!(status_of(&st, &h), "awaiting_answer");
    (st, h, w)
}

fn kinds(h: &FakeHost) -> Vec<&'static str> {
    h.notices().iter().map(|n| n.kind).collect()
}

#[test]
fn whoami_says_whether_the_hub_is_on() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    for session in [INTAKE, &w] {
        assert_eq!(call(&st, &h, session, "whoami", json!({})).unwrap()["hub"], false);
    }
    h.set_hub(true);
    for session in [INTAKE, &w] {
        assert_eq!(call(&st, &h, session, "whoami", json!({})).unwrap()["hub"], true);
    }
}

#[test]
fn ask_user_needs_the_hub() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    call(&st, &h, &w, "get_task", json!({})).unwrap();
    let e = ask(&st, &h, &w, "Which db?").unwrap_err();
    assert_eq!(e, "ask_user is not available: the Agents HUB is off. Ask in plain text and end your turn.");
    assert!(task(&st, &h, "a")["questions"].is_null());
}

#[test]
fn only_the_worker_and_the_planner_may_ask() {
    let (st, h, w) = asker();
    let e = ask(&st, &h, INTAKE, "q").unwrap_err();
    assert!(e.contains("reserved for automated sessions"), "{e}");
    let (rst, rh, r) = reviewer(DeliveryMode::Commit, false);
    rh.set_hub(true);
    let e = ask(&rst, &rh, &r, "q").unwrap_err();
    assert!(e.contains("not available to automated sessions"), "{e}");
    assert!(ask(&st, &h, &w, "q").is_ok());
    let ph = FakeHost::with_delivery(DeliveryMode::Commit, false);
    ph.set_hub(true);
    let pst = AutomationState::default();
    let (_, p) = started_plan(&pst, &ph);
    call(&pst, &ph, &p, "get_task", json!({})).unwrap();
    let reply = ask(&pst, &ph, &p, "Which theme?").unwrap();
    assert_eq!(reply["question_id"], "q-1-1");
}

#[test]
fn the_reply_names_the_question_and_tells_the_worker_to_end_the_turn() {
    let (st, h, w) = asker();
    let reply = call(&st, &h, &w, "ask_user", json!({ "question": " Which db? ", "options": ["Postgres", "SQLite"] }));
    assert_eq!(reply.unwrap(), json!({ "question_id": "q-1-1", "message": ASK_USER_SENT }));
    let t = task(&st, &h, "a");
    assert_eq!(t["question"]["text"], "Which db?");
    assert_eq!(t["question"]["options"], json!(["Postgres", "SQLite"]));
    assert_eq!(t["attention"], false);
    assert_eq!(t["worker_status"], "running", "the worker keeps working until it ends the turn");
    assert!(h.notices().is_empty(), "no notification when the question is asked");
}

#[test]
fn a_bad_question_is_rejected_and_nothing_is_recorded() {
    let (st, h, w) = asker();
    let long_option = "x".repeat(OPTION_MAX + 1);
    let many: Vec<String> = (0..=OPTIONS_MAX).map(|i| format!("o{i}")).collect();
    let cases = [
        (json!({}), "question is missing"),
        (json!({ "question": "  " }), "question is missing"),
        (json!({ "question": "x".repeat(QUESTION_MAX + 1) }), "too long"),
        (json!({ "question": "q", "options": many }), "too many options"),
        (json!({ "question": "q", "options": [long_option] }), "too long"),
        (json!({ "question": "q", "options": ["a\nb"] }), "single line"),
        (json!({ "question": "q", "options": ["a", "a"] }), "duplicate option"),
    ];
    for (params, needle) in cases {
        let e = call(&st, &h, &w, "ask_user", params.clone()).unwrap_err();
        assert!(e.contains(needle), "{params}: {e}");
    }
    assert!(task(&st, &h, "a")["questions"].is_null());
}

#[test]
fn a_second_question_is_refused_while_the_first_waits() {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "first").unwrap();
    let e = ask(&st, &h, &w, "second").unwrap_err();
    assert!(e.starts_with("a question is already waiting for the user's answer"), "{e}");
    assert_eq!(task(&st, &h, "a")["questions"].as_array().unwrap().len(), 1);
}

#[test]
fn a_question_ends_the_turn_without_blocking_the_stop_and_notifies_the_user() {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "Which db?").unwrap();
    // Neanche con lo Stop hook gia' sollecitato il worker e' abbandonato: aspetta la risposta.
    assert_eq!(stop(&st, &h, &w, Some(true), None), None);
    assert_eq!(status_of(&st, &h), "awaiting_answer");
    assert_eq!(task(&st, &h, "a")["attention"], false);
    let notices = h.notices();
    assert_eq!(kinds(&h), ["question"]);
    assert_eq!(notices[0].body, "Task a ha una domanda");
    assert_eq!(notices[0].tab_id.as_deref(), Some(w.as_str()));
    // Un secondo Stop non notifica di nuovo.
    assert_eq!(stop(&st, &h, &w, Some(false), None), None);
    assert_eq!(kinds(&h), ["question"]);
}

#[test]
fn a_stop_without_a_question_is_still_blocked() {
    let (st, h, w) = asker();
    assert!(stop(&st, &h, &w, Some(false), None).is_some());
    assert_eq!(status_of(&st, &h), "running");
}

#[test]
fn the_answer_is_typed_in_the_terminal_and_then_submitted() {
    let (st, h, w) = waiting();
    h.hold_defers();
    answer(&st, &h, "q-1-1", "Postgres").unwrap();
    assert_eq!(h.tab_writes(), [(w.clone(), b"Postgres".to_vec())], "the submit waits for the delay");
    assert!(h.log().contains(&"defer:150".to_string()));
    h.run_held();
    assert_eq!(h.tab_writes(), [(w.clone(), b"Postgres".to_vec()), (w.clone(), b"\r".to_vec())]);
    let t = task(&st, &h, "a");
    assert_eq!(t["questions"][0]["answer"], "Postgres");
    assert!(t["questions"][0]["answered_at"].is_i64());
    assert!(t.get("question").is_none());
    assert_eq!(t["worker_status"], "running");
    assert_eq!(kinds(&h), ["question"]);
}

#[test]
fn a_multi_line_answer_is_pasted_after_being_sanitized() {
    let (st, h, w) = waiting();
    answer(&st, &h, "q-1-1", "  line one\r\nline\x1b[201~ two\t!  ").unwrap();
    let writes = h.tab_writes();
    assert_eq!(writes[0], (w.clone(), b"\x1b[200~line one\nline[201~ two !\x1b[201~".to_vec()));
    assert_eq!(writes[1], (w, b"\r".to_vec()));
    assert_eq!(task(&st, &h, "a")["questions"][0]["answer"], "line one\nline[201~ two !");
}

#[test]
fn the_worker_can_ask_again_after_the_answer() {
    let (st, h, w) = waiting();
    answer(&st, &h, "q-1-1", "Postgres").unwrap();
    let reply = ask(&st, &h, &w, "And the port?").unwrap();
    assert_eq!(reply["question_id"], "q-1-2");
    assert_eq!(stop(&st, &h, &w, Some(false), None), None);
    assert_eq!(status_of(&st, &h), "awaiting_answer");
    assert_eq!(kinds(&h), ["question", "question"]);
}

#[test]
fn an_answer_is_refused_while_the_worker_waits_for_a_permission() {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "Which db?").unwrap();
    hook(&st, &h, &w, "needs-input");
    let e = answer(&st, &h, "q-1-1", "Postgres").unwrap_err();
    assert_eq!(e, "Il worker aspetta un permesso nel terminale: rispondi lì prima di inviare la risposta.");
    assert!(h.tab_writes().is_empty());
    assert!(task(&st, &h, "a")["question"].is_object(), "the question stays pending");
}

#[test]
fn an_answer_is_refused_while_the_worker_is_still_working() {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "Which db?").unwrap();
    let e = answer(&st, &h, "q-1-1", "Postgres").unwrap_err();
    assert_eq!(e, "Il worker sta ancora lavorando: la risposta si può inviare quando si ferma in attesa.");
    assert!(h.tab_writes().is_empty());
    assert!(task(&st, &h, "a")["question"].is_object());
}

#[test]
fn an_answer_is_refused_when_the_tab_is_closed_and_the_question_stays_recorded() {
    let (st, h, w) = waiting();
    tab_closed(&st, &h, &w);
    let e = answer(&st, &h, "q-1-1", "Postgres").unwrap_err();
    assert_eq!(e, "Il worker del task non è più attivo (tab chiuso o lavoro finito): la domanda resta registrata.");
    assert!(h.tab_writes().is_empty());
    let t = task(&st, &h, "a");
    assert_eq!(t["questions"][0]["text"], "Which db?");
    assert!(t["questions"][0].get("answered_at").is_none() && t.get("question").is_none());
}

#[test]
fn an_answer_names_a_known_task_and_question_once() {
    let (st, h, _) = waiting();
    assert_eq!(st.answer_question(&h, "ws-1", "zzz", "q-1-1", "x").unwrap_err(), "Task non trovato.");
    assert_eq!(st.answer_question(&h, "ws-9", "a", "q-1-1", "x").unwrap_err(), "Task non trovato.");
    assert_eq!(answer(&st, &h, "q-1-9", "x").unwrap_err(), "Domanda non trovata.");
    answer(&st, &h, "q-1-1", "Postgres").unwrap();
    assert_eq!(answer(&st, &h, "q-1-1", "again").unwrap_err(), "La domanda ha già una risposta.");
    assert_eq!(h.tab_writes().len(), 2, "only the first answer reached the terminal");
}

#[test]
fn an_empty_or_too_long_answer_is_refused_before_anything_changes() {
    let (st, h, _) = waiting();
    for blank in ["", "   ", "\x1b\x07\n\t"] {
        assert_eq!(answer(&st, &h, "q-1-1", blank).unwrap_err(), "La risposta è vuota.");
    }
    let long = "x".repeat(ANSWER_MAX + 1);
    assert_eq!(answer(&st, &h, "q-1-1", &long).unwrap_err(), "La risposta è troppo lunga (massimo 4000 caratteri).");
    assert!(answer(&st, &h, "q-1-1", &"x".repeat(ANSWER_MAX)).is_ok());
}

#[test]
fn a_failed_write_puts_the_question_back_to_waiting() {
    let (st, h, _) = waiting();
    h.fail_tab_writes();
    let e = answer(&st, &h, "q-1-1", "Postgres").unwrap_err();
    assert_eq!(e, "Il terminale del worker non è raggiungibile: la domanda resta in attesa.");
    let t = task(&st, &h, "a");
    assert!(t["question"].is_object() && t["questions"][0].get("answer").is_none());
    assert_eq!(t["worker_status"], "awaiting_answer");
    assert!(h.tab_writes().is_empty());
}

#[test]
fn the_user_answering_in_the_terminal_closes_the_question_without_text() {
    let (st, h, w) = waiting();
    st.run(&h, |r| r.user_input(&w, 2));
    let t = task(&st, &h, "a");
    assert_eq!(t["worker_status"], "running");
    assert!(t.get("question").is_none());
    assert!(t["questions"][0]["answered_at"].is_i64() && t["questions"][0].get("answer").is_none());
    assert!(h.tab_writes().is_empty());
}

#[test]
fn enter_on_a_permission_prompt_does_not_close_the_question() {
    let (st, h, w) = asker();
    ask(&st, &h, &w, "Which db?").unwrap();
    hook(&st, &h, &w, "needs-input");
    st.run(&h, |r| r.user_input(&w, 2));
    let t = task(&st, &h, "a");
    assert_eq!(t["worker_status"], "running", "the permission was approved");
    assert!(t["question"].is_object(), "the question still waits");
    // Chiuso il turno, il worker torna ad aspettare la risposta.
    assert_eq!(stop(&st, &h, &w, Some(false), None), None);
    assert_eq!(status_of(&st, &h), "awaiting_answer");
}

#[test]
fn a_permission_prompt_wins_over_the_wait_for_the_answer() {
    let (st, h, w) = waiting();
    hook(&st, &h, &w, "needs-input");
    assert_eq!(status_of(&st, &h), "needs_input");
    assert_eq!(kinds(&h), ["question", "attention"]);
}

#[test]
fn the_ask_user_instructions_appear_only_with_the_hub_on_and_never_for_the_reviewer() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let instr = |st: &AutomationState, h: &FakeHost, s: &str| {
        call(st, h, s, "get_task", json!({})).unwrap()["instructions"].as_str().unwrap().to_string()
    };
    let off = instr(&st, &h, &w);
    assert!(!off.contains("ask_user") && off.contains("ask the question in plain text"));
    h.set_hub(true);
    let on = instr(&st, &h, &w);
    assert!(on.contains(ASK_WITH_TOOL) && !on.contains("ask the question in plain text"));

    let ph = FakeHost::with_delivery(DeliveryMode::Commit, false);
    let pst = AutomationState::default();
    let (_, p) = started_plan(&pst, &ph);
    assert!(!instr(&pst, &ph, &p).contains("ask_user"));
    ph.set_hub(true);
    assert!(instr(&pst, &ph, &p).contains(PLANNER_ASK_USER.trim_end()));

    let (rst, rh, r) = reviewer(DeliveryMode::Commit, false);
    let before = instr(&rst, &rh, &r);
    rh.set_hub(true);
    assert_eq!(instr(&rst, &rh, &r), before);
    assert!(!before.contains("ask_user"));
}
