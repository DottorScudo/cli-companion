//! Lo stato di un worker e' derivato dal suo task (finche' non e' finito) e il
//! registro sopravvive a un panico di un altro thread.

use super::*;
use crate::board::TaskAction;

fn status_of(st: &AutomationState, session: &str) -> Option<WorkerStatus> {
    st.run(&FakeHost::default(), |r| r.worker_status_of(session))
}

#[test]
fn the_worker_status_follows_the_task_and_ends_with_the_worker() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Starting));
    call(&st, &h, &w, "get_task", json!({})).unwrap();
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Running));
    hook(&st, &h, &w, "needs-input");
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::NeedsInput));
    assert_eq!(task(&st, &h, "a")["worker_status"], "needs_input", "il task e il worker dicono la stessa cosa");
    st.run(&h, |r| r.user_input(&w, 2));
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Running));
    call(&st, &h, &w, "complete_task", json!({"status": "completed", "summary": "ok"})).unwrap();
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Completed));
    assert!(task(&st, &h, "a")["worker_status"].is_null(), "il task concluso non ha piu' un worker");
    // Un worker finito ignora gli hook.
    assert_eq!(stop(&st, &h, &w, Some(false), None), None);
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Completed));
    assert_eq!(status_of(&st, "auto-unknown"), None);
}

#[test]
fn a_worker_whose_task_is_no_longer_its_own_is_closed() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    *h.archived.lock().unwrap() = true;
    st.set_archived(&h, "ws-1", true);
    assert_eq!(task(&st, &h, "a")["status"], "queued");
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Closed));
    let failed = st.run(&h, |r| r.live_worker(&w).map(|_| ()));
    assert_eq!(failed, Err("the session is closed".to_string()));
}

#[test]
fn stop_closes_the_worker_before_the_task_fails() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    boards::task_action(&st, &h, "ws-1", "a", TaskAction::Stop).unwrap();
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::Closed));
    assert!(h.log().contains(&format!("update:{w}:Closed")));
}

#[test]
fn a_panic_while_holding_the_registry_does_not_break_the_app() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let poisoner = st.clone();
    let result = std::thread::spawn(move || {
        let _held = poisoner.registry();
        panic!("simulated panic while holding the registry");
    })
    .join();
    assert!(result.is_err());
    assert!(st.inner.is_poisoned());
    // Tutto continua a funzionare: accessi in lettura, comandi e hook.
    assert_eq!(st.open_tabs().len(), 1);
    assert_eq!(view(&st, &h).tasks.len(), 1);
    hook(&st, &h, &w, "needs-input");
    assert_eq!(status_of(&st, &w), Some(WorkerStatus::NeedsInput));
    boards::task_action(&st, &h, "ws-1", "a", TaskAction::Stop).unwrap();
    assert_eq!(task(&st, &h, "a")["status"], "failed");
}
