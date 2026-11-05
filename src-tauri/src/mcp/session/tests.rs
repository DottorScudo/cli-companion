use super::super::retry::RETRY_POLICY;
use super::super::UnavailableBackend;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const LIST_CHANGED: &str = "notifications/tools/list_changed";

fn tools_list(id: u32) -> String {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/list"}).to_string()
}

/// App che non risponde a `whoami` per le prime `failures` chiamate, poi
/// risponde con il ruolo di un worker.
struct Flaky {
    failures: AtomicUsize,
    whoami: AtomicUsize,
}

impl Flaky {
    fn failing(failures: usize) -> Arc<Self> {
        Arc::new(Self { failures: AtomicUsize::new(failures), whoami: AtomicUsize::new(0) })
    }

    /// Da ora in poi `whoami` riesce.
    fn come_up(&self) {
        self.failures.store(0, Ordering::SeqCst);
    }

    fn attempts(&self) -> usize {
        self.whoami.load(Ordering::SeqCst)
    }
}

impl Backend for Flaky {
    fn rpc(&self, method: &str, _params: &Value, _timeout: Duration) -> Result<Value, String> {
        assert_eq!(method, "whoami", "only the role is asked in these tests");
        self.whoami.fetch_add(1, Ordering::SeqCst);
        let failed = self.failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
        if failed {
            Err("timeout waiting for the response".to_string())
        } else {
            Ok(json!({ "role": "worker", "session": "t1" }))
        }
    }
}

/// Orologio virtuale: `sleep` registra l'attesa e sposta il tempo, subito.
struct FakeClock {
    now: Mutex<Instant>,
    sleeps: Mutex<Vec<Duration>>,
}

impl FakeClock {
    fn new() -> Arc<Self> {
        Arc::new(Self { now: Mutex::new(Instant::now()), sleeps: Mutex::default() })
    }

    fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().unwrap().clone()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }

    fn sleep(&self, d: Duration) {
        self.sleeps.lock().unwrap().push(d);
        *self.now.lock().unwrap() += d;
    }
}

struct Harness {
    session: Session<Vec<u8>>,
    out: Arc<Mutex<Vec<u8>>>,
}

impl Harness {
    fn new(backend: Arc<dyn Backend>) -> Self {
        let out = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(backend, Arc::clone(&out), FakeClock::new(), RETRY_POLICY);
        Self { session, out }
    }

    fn respond(&self, line: &str) -> Option<RetryJob<Vec<u8>>> {
        self.session.respond(line).expect("writing to a Vec never fails")
    }

    /// Ogni riga scritta finora, come JSON (fallisce se una riga non lo e').
    fn messages(&self) -> Vec<Value> {
        let out = self.out.lock().unwrap();
        let text = std::str::from_utf8(&out).unwrap();
        assert!(text.is_empty() || text.ends_with('\n'), "every message is a whole line");
        text.lines().map(|l| serde_json::from_str(l).expect("only JSON messages on stdout")).collect()
    }

    fn notifications(&self) -> usize {
        self.messages().iter().filter(|m| m["method"] == LIST_CHANGED).count()
    }
}

fn tool_count(msg: &Value) -> usize {
    msg["result"]["tools"].as_array().expect("a tools/list response").len()
}

/// Fa girare i tentativi su un thread vero, come in produzione, e li aspetta.
fn run_in_background(job: RetryJob<Vec<u8>>) {
    thread::spawn(move || job.run()).join().unwrap();
}

#[test]
fn a_late_role_sends_one_list_changed_and_the_next_list_has_the_tools() {
    let backend = Flaky::failing(usize::MAX);
    let h = Harness::new(backend.clone());
    let job = h.respond(INITIALIZE).expect("a role missing at the handshake starts the retries");
    assert!(h.respond(INITIALIZED).is_none());
    assert!(h.respond(&tools_list(2)).is_none());
    let before = h.messages();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0]["result"]["capabilities"]["tools"]["listChanged"], true);
    assert_eq!(tool_count(&before[1]), 0);

    backend.come_up();
    run_in_background(job);
    let after = h.messages();
    assert_eq!(after.len(), 3);
    assert_eq!(after[2], json!({ "jsonrpc": "2.0", "method": LIST_CHANGED }));

    h.respond(&tools_list(3));
    let last = h.messages();
    assert_eq!(last[3]["id"], 3);
    assert!(tool_count(&last[3]) > 0);
    assert_eq!(h.notifications(), 1);
}

#[test]
fn the_list_changed_waits_for_notifications_initialized() {
    let backend = Flaky::failing(usize::MAX);
    let h = Harness::new(backend.clone());
    let job = h.respond(INITIALIZE).unwrap();
    // Client fuori regola: chiede la lista prima di `initialized`.
    h.respond(&tools_list(2));
    backend.come_up();
    run_in_background(job);
    assert_eq!(h.notifications(), 0, "no notification before notifications/initialized");
    h.respond(INITIALIZED);
    let messages = h.messages();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2]["method"], LIST_CHANGED);
}

#[test]
fn no_list_changed_when_the_role_was_known_before_the_list() {
    // Ruolo arrivato in background prima della lista.
    let backend = Flaky::failing(usize::MAX);
    let h = Harness::new(backend.clone());
    let job = h.respond(INITIALIZE).unwrap();
    h.respond(INITIALIZED);
    backend.come_up();
    run_in_background(job);
    h.respond(&tools_list(2));
    assert!(tool_count(&h.messages()[1]) > 0);
    assert_eq!(h.notifications(), 0);

    // Ruolo risolto all'handshake: nessun tentativo, nessuna notifica.
    let h = Harness::new(Flaky::failing(0));
    assert!(h.respond(INITIALIZE).is_none());
    h.respond(INITIALIZED);
    h.respond(&tools_list(2));
    assert_eq!(h.messages().len(), 2);
    assert_eq!(h.notifications(), 0);
}

#[test]
fn outside_cli_companion_there_are_no_retries_and_no_notifications() {
    let h = Harness::new(Arc::new(UnavailableBackend("not set".to_string())));
    assert!(h.respond(INITIALIZE).is_none());
    assert!(h.respond(INITIALIZED).is_none());
    assert!(h.respond(&tools_list(2)).is_none());
    let messages = h.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(tool_count(&messages[1]), 0);
    assert_eq!(h.notifications(), 0);
}

#[test]
fn retries_back_off_and_stop_after_the_budget() {
    let backend = Flaky::failing(usize::MAX);
    let clock = FakeClock::new();
    assert!(!retry_role(&Server::default(), backend.as_ref(), clock.as_ref(), &RETRY_POLICY));
    let sleeps = clock.sleeps();
    let secs = |s: u64| Duration::from_secs(s);
    assert_eq!(sleeps[..6], [secs(1), secs(2), secs(4), secs(8), secs(10), secs(10)]);
    assert!(sleeps.iter().all(|d| *d <= RETRY_POLICY.max));
    assert_eq!(sleeps.iter().sum::<Duration>(), RETRY_POLICY.budget, "the last wait is clipped to the budget");
    assert_eq!(backend.attempts(), sleeps.len(), "one whoami after every wait, none after the budget");

    // Il lavoro in background finisce senza scrivere nulla.
    let h = Harness::new(backend.clone());
    let job = h.respond(INITIALIZE).unwrap();
    h.respond(INITIALIZED);
    h.respond(&tools_list(2));
    run_in_background(job);
    assert_eq!(h.messages().len(), 2);
}

#[test]
fn retries_end_as_soon_as_the_role_is_known() {
    let backend = Flaky::failing(0);
    let server = Server::default();
    server.role(backend.as_ref()).unwrap();
    let clock = FakeClock::new();
    assert!(retry_role(&server, backend.as_ref(), clock.as_ref(), &RETRY_POLICY));
    assert_eq!(clock.sleeps(), [RETRY_POLICY.first]);
    assert_eq!(backend.attempts(), 1, "the cached role needs no new whoami");
}

#[test]
fn racing_list_and_retry_never_lose_or_misplace_the_notification() {
    for _ in 0..100 {
        // initialize fallisce; dei due whoami che poi corrono (tools/list e
        // il primo tentativo) uno fallisce e l'altro riesce.
        let h = Harness::new(Flaky::failing(2));
        let job = h.respond(INITIALIZE).unwrap();
        h.respond(INITIALIZED);
        let retry = thread::spawn(move || job.run());
        h.respond(&tools_list(2));
        retry.join().unwrap();
        let messages = h.messages();
        let list = messages.iter().position(|m| m["id"] == 2).unwrap();
        let empty = tool_count(&messages[list]) == 0;
        assert_eq!(h.notifications(), usize::from(empty));
        if empty {
            assert_eq!(messages.last().unwrap()["method"], LIST_CHANGED, "the notification follows the empty list");
        }
    }
}
