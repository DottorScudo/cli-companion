use super::*;
use crate::agents::testing::{FakeIntegration, FAKE_WAIT_APPROVAL, FAKE_WAIT_ASK, FAKE_WAIT_NUDGE, FAKE_WAIT_PREFIX};
use crate::agents::{HookEvents, WaitKind};
use crate::ipc::client::{build_payload, rpc_call, send_for_reply, send_to};
use crate::ipc::wire::hook_kind;

fn payload(event: &str, token: Option<&str>) -> NotifyPayload {
    NotifyPayload {
        event: event.to_string(),
        agent: "fake".to_string(),
        tab_id: Some("tab-1".to_string()),
        agent_session_id: None,
        notification_type: None,
        message: None,
        token: token.map(str::to_string),
        stop_hook_active: None,
        background_work: None,
        source: None,
        reason: None,
        transcript_path: None,
        tool_name: None,
    }
}

/// Credenziale finta del self-test.
struct FakeCredential;

impl Credential for FakeCredential {
    fn session(&self) -> &str {
        "probe-fake"
    }

    fn secret(&self) -> &str {
        "probe-secret"
    }
}

/// Handler finto: ricorda cosa riceve e blocca gli stop se `block` e' impostato.
struct FakeHandler {
    block: Option<String>,
    events: Mutex<Vec<NotifyPayload>>,
}

impl FakeHandler {
    fn new(block: Option<&str>) -> Self {
        Self { block: block.map(str::to_string), events: Mutex::default() }
    }

    fn received(&self) -> Vec<NotifyPayload> {
        self.events.lock().unwrap().clone()
    }
}

impl IpcHandler for FakeHandler {
    fn authenticate(&self, session: &str, secret: &str) -> bool {
        session == "tab-1" && secret == "secret"
    }

    fn probe_credential(&self) -> Result<Box<dyn Credential>, String> {
        Ok(Box::new(FakeCredential))
    }

    fn rpc(&self, req: &RpcRequest) -> RpcResponse {
        RpcResponse::success(serde_json::json!({"echo": req.method}))
    }

    fn hook_event(&self, payload: &NotifyPayload) -> Option<String> {
        self.events.lock().unwrap().push(payload.clone());
        self.block.clone()
    }
}

/// Esegue `process_notify` e ritorna la risposta con i payload inoltrati al frontend.
fn run(handler: &FakeHandler, p: NotifyPayload) -> (Result<NotifyReply, String>, Vec<NotifyPayload>) {
    let gate = Mutex::new(NotifyGate::default());
    run_with(&gate, handler, p)
}

fn run_with(
    gate: &Mutex<NotifyGate>,
    handler: &FakeHandler,
    p: NotifyPayload,
) -> (Result<NotifyReply, String>, Vec<NotifyPayload>) {
    let mut forwarded = Vec::new();
    let reply = process_notify(gate, handler, p, Instant::now(), |p| {
        forwarded.push(p.clone());
        Ok(())
    });
    (reply, forwarded)
}

#[test]
fn events_without_the_right_token_reach_nobody() {
    let handler = FakeHandler::new(Some("never"));
    for token in [None, Some(""), Some("wrong")] {
        let (reply, forwarded) = run(&handler, payload("finished", token));
        assert_eq!(reply, Ok(NotifyReply::default()), "{token:?}");
        assert!(forwarded.is_empty(), "{token:?}");
    }
    assert!(handler.received().is_empty());
}

#[test]
fn events_with_the_secret_of_another_tab_or_without_a_tab_reach_nobody() {
    let handler = FakeHandler::new(Some("never"));
    let mut other_tab = payload("finished", Some("secret"));
    other_tab.tab_id = Some("tab-2".to_string());
    let mut no_tab = payload("finished", Some("secret"));
    no_tab.tab_id = None;
    for p in [other_tab, no_tab] {
        let (reply, forwarded) = run(&handler, p);
        assert_eq!(reply, Ok(NotifyReply::default()));
        assert!(forwarded.is_empty());
    }
    assert!(handler.received().is_empty());
}

#[test]
fn a_valid_event_reaches_the_handler_and_the_frontend_without_the_token() {
    let handler = FakeHandler::new(None);
    let (reply, forwarded) = run(&handler, payload("needs-input", Some("secret")));
    assert_eq!(reply, Ok(NotifyReply::default()));
    let expected = payload("needs-input", None);
    assert_eq!(handler.received(), vec![expected.clone()]);
    assert_eq!(forwarded, vec![expected]);
}

#[test]
fn permission_requests_are_rewritten_before_the_handler_sees_them() {
    let handler = FakeHandler::new(None);
    let gate = Mutex::new(NotifyGate::default());
    let (_, forwarded) = run_with(&gate, &handler, payload("permission-request", Some("secret")));
    assert_eq!(handler.received()[0].event, "needs-input");
    assert_eq!(forwarded[0].wait_kind(), Some(WaitKind::Permission));
    // Il duplicato che segue non raggiunge ne' l'handler ne' il frontend.
    let mut dup = payload("needs-input", Some("secret"));
    dup.set_wait_kind(Some(WaitKind::Permission));
    let (reply, forwarded) = run_with(&gate, &handler, dup);
    assert_eq!(reply, Ok(NotifyReply::default()));
    assert!(forwarded.is_empty());
    assert_eq!(handler.received().len(), 1);
}

/// Payload di `--notify <event>` con lo stdin `raw` dell'integrazione finta,
/// come lo spedisce il client.
fn from_fake_agent(event: &str, raw: &str) -> NotifyPayload {
    let kind = hook_kind(event).expect("evento di un hook");
    let details = FakeIntegration::full("fake").normalize(kind, raw);
    NotifyPayload { token: Some("secret".to_string()), ..build_payload(event, "fake", Some("tab-1"), details) }
}

#[test]
fn an_agent_with_its_own_notification_names_gets_the_same_rewrite_and_dedupe() {
    let handler = FakeHandler::new(None);
    let gate = Mutex::new(NotifyGate::default());
    let wait = |name: &str| format!("{FAKE_WAIT_PREFIX}{name}");
    // L'attesa inattiva arriva come tale, senza aprire la finestra del duplicato.
    let (_, forwarded) = run_with(&gate, &handler, from_fake_agent("needs-input", &wait(FAKE_WAIT_NUDGE)));
    assert_eq!(forwarded[0].wait_kind(), Some(WaitKind::Idle));
    let (_, forwarded) = run_with(&gate, &handler, from_fake_agent("needs-input", &wait(FAKE_WAIT_APPROVAL)));
    assert_eq!(forwarded[0].wait_kind(), Some(WaitKind::Permission));
    // Il permesso segnalato con il dettaglio scarta il needs-input che lo ripete.
    let (_, forwarded) = run_with(&gate, &handler, from_fake_agent("permission-request", ""));
    assert_eq!((forwarded[0].event.as_str(), forwarded[0].wait_kind()), ("needs-input", Some(WaitKind::Permission)));
    let (reply, forwarded) = run_with(&gate, &handler, from_fake_agent("needs-input", &wait(FAKE_WAIT_APPROVAL)));
    assert_eq!(reply, Ok(NotifyReply::default()));
    assert!(forwarded.is_empty());
    // Una domanda all'utente non e' un duplicato del permesso.
    let (_, forwarded) = run_with(&gate, &handler, from_fake_agent("needs-input", &wait(FAKE_WAIT_ASK)));
    assert_eq!(forwarded[0].wait_kind(), Some(WaitKind::Question));
    let waits: Vec<_> = handler.received().iter().map(NotifyPayload::wait_kind).collect();
    let (idle, permission, question) = (Some(WaitKind::Idle), Some(WaitKind::Permission), Some(WaitKind::Question));
    assert_eq!(waits, vec![idle, permission, permission, question]);
}

#[test]
fn a_blocked_stop_is_replied_but_not_forwarded() {
    let handler = FakeHandler::new(Some("keep going"));
    let (reply, forwarded) = run(&handler, payload("finished", Some("secret")));
    assert_eq!(reply, Ok(NotifyReply { block: Some("keep going".to_string()) }));
    assert!(forwarded.is_empty());
    assert_eq!(handler.received().len(), 1);
    // Uno stop con lavoro in background e' in pausa anche senza blocco.
    let handler = FakeHandler::new(None);
    let mut busy = payload("finished", Some("secret"));
    busy.background_work = Some(2);
    let (reply, forwarded) = run(&handler, busy);
    assert_eq!(reply, Ok(NotifyReply::default()));
    assert!(forwarded.is_empty());
    // Uno stop vero va al frontend.
    let (_, forwarded) = run(&handler, payload("finished", Some("secret")));
    assert_eq!(forwarded.len(), 1);
}

#[test]
fn a_forward_failure_is_an_error() {
    let handler = FakeHandler::new(None);
    let gate = Mutex::new(NotifyGate::default());
    let r = process_notify(&gate, &handler, payload("finished", Some("secret")), Instant::now(), |_| {
        Err("finestra chiusa".to_string())
    });
    assert_eq!(r, Err("finestra chiusa".to_string()));
}

#[test]
fn binding_falls_back_to_a_free_port_when_the_preferred_one_is_taken() {
    let taken = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let taken_port = taken.local_addr().unwrap().port();
    let bound = bind_listener(taken_port);
    let fallback = bound.as_ref().unwrap().local_addr().unwrap().port();
    assert_ne!(fallback, taken_port);
    assert_eq!(
        status_of(taken_port, &bound),
        IpcStatus { port: Some(fallback), preferred_port: taken_port, error: None }
    );
    // La porta di ripiego funziona davvero.
    let listener = bound.unwrap();
    let client = std::thread::spawn(move || send_to(fallback, &payload("test", None)));
    let (mut s, _) = listener.accept().unwrap();
    assert!(matches!(read_wire(&mut s), Ok(Wire::Notify(_))));
    client.join().unwrap().unwrap();
}

#[test]
fn binding_uses_the_preferred_port_when_it_is_free() {
    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let bound = bind_listener(port);
    assert_eq!(bound.as_ref().unwrap().local_addr().unwrap().port(), port);
    assert_eq!(status_of(port, &bound), IpcStatus { port: Some(port), preferred_port: port, error: None });
}

#[test]
fn a_failed_bind_is_reported_in_the_status() {
    let failed = Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "negato"));
    let status = status_of(17421, &failed);
    assert_eq!((status.port, status.preferred_port), (None, 17421));
    assert!(status.error.unwrap().contains("negato"));
}

#[test]
fn status_before_the_listener_starts_says_so() {
    // Nei test il listener non parte mai: lo stato globale resta vuoto.
    assert_eq!(listening_port(), None);
    let status = ipc_status();
    assert_eq!((status.port, status.preferred_port), (None, DEFAULT_PORT));
    assert_eq!(status.error.as_deref(), Some("listener non avviato"));
    assert_eq!(
        serde_json::to_value(&status).unwrap(),
        serde_json::json!({"port": null, "preferred_port": DEFAULT_PORT, "error": "listener non avviato"})
    );
}

#[test]
fn connection_slots_are_limited_and_released_on_drop() {
    let slots = ConnectionSlots::new(2);
    let a = slots.try_acquire().unwrap();
    let b = slots.try_acquire().unwrap();
    assert!(slots.try_acquire().is_none());
    drop(a);
    let c = slots.try_acquire().expect("un posto liberato torna disponibile");
    assert!(slots.try_acquire().is_none());
    drop((b, c));
    assert_eq!(slots.used.load(Ordering::SeqCst), 0);
    // Una copia condivide i posti.
    let copy = slots.clone();
    let _held = (slots.try_acquire().unwrap(), slots.try_acquire().unwrap());
    assert!(copy.try_acquire().is_none());
    assert_eq!(MAX_CONNECTIONS, 32);
}

/// Roundtrip completo su una porta effimera: il lato server e' il vero
/// `serve_conn` con un handler finto che dura piu' del timeout di lettura
/// della richiesta (2 s), per provare che non si applica alla risposta.
#[test]
fn rpc_roundtrip_over_tcp_with_slow_handler() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        serve_conn(
            &mut s,
            |req| {
                std::thread::sleep(Duration::from_millis(2300));
                RpcResponse::success(serde_json::json!({"echo": req.method, "session": req.session}))
            },
            |_| panic!("attesa una RPC"),
        )
    });
    let line = r#"{"rpc":1,"token":"t","session":"tab-1","method":"ping","params":{}}"#;
    let resp = rpc_call(port, line, Duration::from_secs(10)).unwrap();
    let r: RpcResponse = serde_json::from_str(&resp).unwrap();
    assert!(r.ok);
    assert_eq!(r.result["echo"], "ping");
    assert_eq!(r.result["session"], "tab-1");
    server.join().unwrap().unwrap();
}

#[test]
fn rpc_goes_to_the_handler() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let handler = FakeHandler::new(None);
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        serve_conn(&mut s, |req| handler.rpc(&req), |_| panic!("attesa una RPC"))
    });
    let resp = rpc_call(port, r#"{"rpc":1,"method":"whoami"}"#, Duration::from_secs(5)).unwrap();
    assert_eq!(resp, r#"{"ok":true,"result":{"echo":"whoami"}}"#);
    server.join().unwrap().unwrap();
}

#[test]
fn notify_goes_to_notify_handler() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let sent = payload("needs-input", Some("secret"));
    send_to(port, &sent).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    let mut got = None;
    serve_conn(
        &mut s,
        |_| panic!("attesa una notifica"),
        |p| {
            got = Some(p);
            Ok(NotifyReply::default())
        },
    )
    .unwrap();
    assert_eq!(got.unwrap(), sent);
}

#[test]
fn serve_conn_replies_to_notify_and_tolerates_legacy_client() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let finished = payload("finished", Some("secret"));
    // Client nuovo: legge la riga con il blocco.
    let sent = finished.clone();
    let client = std::thread::spawn(move || send_for_reply(port, &sent, Duration::from_secs(2)).unwrap());
    let (mut s, _) = listener.accept().unwrap();
    serve_conn(&mut s, |_| panic!("attesa una notifica"), |_| Ok(NotifyReply { block: Some("r".into()) })).unwrap();
    assert_eq!(client.join().unwrap(), Some(NotifyReply { block: Some("r".into()) }));
    // Client vecchio: invia e chiude subito; il server non deve fallire.
    send_to(port, &finished).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    serve_conn(&mut s, |_| panic!("attesa una notifica"), |_| Ok(NotifyReply { block: Some("r".into()) })).unwrap();
}

#[test]
fn a_failed_notify_still_answers_with_an_empty_reply() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let finished = payload("finished", Some("secret"));
    let client = std::thread::spawn(move || send_for_reply(port, &finished, Duration::from_secs(2)).unwrap());
    let (mut s, _) = listener.accept().unwrap();
    let r = serve_conn(&mut s, |_| panic!("attesa una notifica"), |_| Err("rotto".to_string()));
    assert_eq!(r, Err("rotto".to_string()));
    assert_eq!(client.join().unwrap(), Some(NotifyReply::default()));
}
