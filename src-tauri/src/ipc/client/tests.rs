use super::*;
use crate::agents::HookEventKind;
use crate::ipc::wire::{parse_line, read_wire, Wire};
use std::net::TcpListener;
use std::sync::Mutex;

/// Contratto finto con un formato di stdin neutro: coppie `chiave=valore`
/// separate da `;`. Registra i tipi di evento con cui viene interrogato.
#[derive(Default)]
struct FakeHooks {
    calls: Mutex<Vec<HookEventKind>>,
}

impl FakeHooks {
    fn kinds(&self) -> Vec<HookEventKind> {
        self.calls.lock().unwrap().clone()
    }
}

impl HookEvents for FakeHooks {
    fn normalize(&self, kind: HookEventKind, raw: &str) -> HookEvent {
        self.calls.lock().unwrap().push(kind);
        let mut e = HookEvent::default();
        for (key, value) in raw.split(';').filter_map(|part| part.split_once('=')) {
            let v = value.to_string();
            match key {
                "id" => e.agent_session_id = Some(v),
                "type" => e.notification_type = Some(v),
                "msg" => e.message = Some(v),
                "cont" => e.already_continuing = Some(value == "1"),
                "bg" => e.background_work = value.parse().ok(),
                "src" => e.start_source = Some(v),
                "why" => e.end_reason = Some(v),
                "path" => e.transcript_path = Some(v),
                "tool" => e.tool_name = Some(v),
                "sum" => e.tool_summary = Some(v),
                _ => {}
            }
        }
        e
    }

    fn stop_block_output(&self, reason: &str) -> String {
        format!("STOP[{reason}]")
    }
}

fn sample(event: &str) -> NotifyPayload {
    build_payload(event, "fake", Some("tab-1"), HookEvent::default())
}

fn finished_with_token() -> NotifyPayload {
    let mut p = sample("finished");
    p.token = Some("tok".to_string());
    p
}

/// Listener sentinella su porta effimera: `accept` non bloccante, cosi'
/// una connessione mancata si verifica senza attese.
fn sentinel() -> (TcpListener, u16) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// Server finto su porta effimera: legge la notifica e risponde `reply`
/// (se `Some`), poi chiude; con `None` resta in silenzio per `hold`.
fn fake_listener(reply: Option<&'static str>, hold: Duration) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let h = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let Wire::Notify(p) = read_wire(&mut s).unwrap() else {
            panic!("attesa una notifica");
        };
        assert_eq!(p.event, "finished");
        match reply {
            Some(r) => {
                s.write_all(format!("{r}\n").as_bytes()).unwrap();
            }
            None => std::thread::sleep(hold),
        }
    });
    (port, h)
}

#[test]
fn wire_roundtrip_over_tcp() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let details = FakeHooks::default().normalize(HookEventKind::NeedsInput, "id=abc;type=ask;msg=Serve una risposta");
    let sent = build_payload("needs-input", "fake", Some("tab-1"), details);
    send_to(port, &sent).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    let Wire::Notify(got) = read_wire(&mut stream).unwrap() else {
        panic!("attesa una notifica");
    };
    assert_eq!(got, sent);
    assert_eq!(got.agent_session_id.as_deref(), Some("abc"));
    assert_eq!(got.message.as_deref(), Some("Serve una risposta"));
}

#[test]
fn sending_to_a_closed_port_or_an_unknown_event_fails() {
    assert!(send_to(1, &sample("test")).is_err()); // porta chiusa
    assert!(send_to(1, &sample("nope")).is_err()); // evento sconosciuto
}

#[test]
fn should_send_decision() {
    // Legacy Muse e sconosciuti: mai.
    assert!(!should_send("stop", Some("tab-1")));
    // `session-end` ora e' un evento vero, ma il vecchio plugin Muse non
    // imposta CLI_COMPANION_SESSION: senza tab resta ignorato.
    assert!(!should_send("session-end", None));
    assert!(should_send("session-end", Some("tab-1")));
    assert!(!should_send("", Some("tab-1")));
    // Test: sempre, anche senza tab.
    assert!(should_send("test", None));
    assert!(should_send("test", Some("tab-1")));
    // Eventi degli agenti: solo con un id tab valido.
    for ev in ["session-start", "session-end", "finished", "needs-input", "permission-request"] {
        assert!(should_send(ev, Some("tab-1")), "{ev}");
        assert!(!should_send(ev, None), "{ev}");
        assert!(!should_send(ev, Some("")), "{ev}");
        assert!(!should_send(ev, Some("a b")), "{ev}");
    }
}

#[test]
fn ignored_events_neither_read_stdin_nor_connect() {
    let (listener, port) = sentinel();
    let hooks = FakeHooks::default();
    let no_stdin = || -> Option<String> { panic!("stdin read for an ignored event") };
    for (event, tab_id) in [("stop", Some("tab-1")), ("session-end", None), ("finished", Some("a b"))] {
        let target = NotifyTarget { tab_id, token: Some("tok"), port };
        let mut out = Vec::new();
        assert!(notify_target(event, "fake", &target, no_stdin, Some(&hooks), &mut out).is_ok(), "{event}");
        assert!(out.is_empty(), "{event}");
    }
    let err = listener.accept().map(|_| ()).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock, "nessuna connessione tentata");
    assert!(hooks.kinds().is_empty());
}

#[test]
fn a_sent_event_carries_the_given_tab_token_and_normalized_details() {
    let (listener, port) = sentinel();
    let hooks = FakeHooks::default();
    let target = NotifyTarget { tab_id: Some("tab-1"), token: Some("tok"), port };
    let stdin = || Some("id=s1;why=other;path=/t/s.jsonl".to_string());
    notify_target("session-end", "fake", &target, stdin, Some(&hooks), &mut Vec::new()).unwrap();
    listener.set_nonblocking(false).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    let Wire::Notify(p) = read_wire(&mut s).unwrap() else {
        panic!("attesa una notifica");
    };
    assert_eq!(
        (p.event.as_str(), p.agent.as_str(), p.tab_id.as_deref(), p.token.as_deref()),
        ("session-end", "fake", Some("tab-1"), Some("tok"))
    );
    assert_eq!((p.agent_session_id.as_deref(), p.reason.as_deref()), (Some("s1"), Some("other")));
    assert_eq!(p.transcript_path.as_deref(), Some("/t/s.jsonl"));
    assert_eq!(hooks.kinds(), vec![HookEventKind::SessionEnd]);
}

#[test]
fn without_a_contract_the_event_goes_out_bare() {
    // Agente senza contratto (`hooks: None`): evento di base, nessun dettaglio.
    let (listener, port) = sentinel();
    let target = NotifyTarget { tab_id: Some("tab-1"), token: Some("tok"), port };
    let stdin = || Some("id=s1;why=other;msg=ciao".to_string());
    notify_target("session-end", "unknown", &target, stdin, None, &mut Vec::new()).unwrap();
    listener.set_nonblocking(false).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    let Wire::Notify(p) = read_wire(&mut s).unwrap() else {
        panic!("attesa una notifica");
    };
    let expected = NotifyPayload {
        token: Some("tok".into()),
        ..build_payload("session-end", "unknown", Some("tab-1"), HookEvent::default())
    };
    assert_eq!(p, expected);
    assert_eq!((p.agent_session_id, p.reason, p.message), (None, None, None));
}

#[test]
fn the_test_event_and_a_missing_stdin_carry_no_details() {
    let hooks = FakeHooks::default();
    // `test` non e' un evento di un agente: il contratto non viene interrogato.
    let (listener, port) = sentinel();
    let target = NotifyTarget { tab_id: None, token: Some("tok"), port };
    notify_target("test", "self-test", &target, || Some("id=x".into()), Some(&hooks), &mut Vec::new()).unwrap();
    listener.set_nonblocking(false).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    let Wire::Notify(p) = read_wire(&mut s).unwrap() else {
        panic!("attesa una notifica");
    };
    assert_eq!((p.event.as_str(), p.tab_id, p.agent_session_id), ("test", None, None));
    // Stdin assente (terminale): nessuna chiamata al contratto.
    let (listener, port) = sentinel();
    let target = NotifyTarget { tab_id: Some("t"), token: None, port };
    notify_target("session-start", "fake", &target, || None, Some(&hooks), &mut Vec::new()).unwrap();
    listener.set_nonblocking(false).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    let Wire::Notify(p) = read_wire(&mut s).unwrap() else {
        panic!("attesa una notifica");
    };
    assert_eq!((p.token, p.agent_session_id, p.source), (None, None, None));
    assert!(hooks.kinds().is_empty());
}

#[test]
fn permission_request_message_is_built_from_tool_and_summary() {
    let hooks = FakeHooks::default();
    let details = hooks.normalize(HookEventKind::PermissionRequest, "id=s1;tool=Tool;sum=do the thing");
    let p = build_payload("permission-request", "fake", Some("tab-1"), details);
    assert!(crate::ipc::wire::is_known_event(&p.event));
    assert_eq!(p.tool_name.as_deref(), Some("Tool"));
    assert_eq!(p.message.as_deref(), Some("Permesso richiesto: Tool \u{2014} do the thing"));
    // Il payload sopravvive al giro sul socket.
    assert_eq!(parse_line(&build_line(&p).unwrap()).unwrap(), p);
    // Tool senza riassunto: solo il nome; senza tool: nessun messaggio.
    assert_eq!(permission_text("Tool", None), "Permesso richiesto: Tool");
    let p = build_payload(
        "permission-request",
        "fake",
        Some("t"),
        hooks.normalize(HookEventKind::PermissionRequest, "tool=Tool"),
    );
    assert_eq!(p.message.as_deref(), Some("Permesso richiesto: Tool"));
    assert_eq!(build_payload("permission-request", "fake", Some("t"), HookEvent::default()).message, None);
    // Il messaggio dell'agente non sostituisce quello del permesso.
    let p = build_payload(
        "permission-request",
        "fake",
        Some("t"),
        hooks.normalize(HookEventKind::PermissionRequest, "msg=altro"),
    );
    assert_eq!(p.message, None);
}

#[test]
fn build_payload_maps_every_detail_to_its_wire_field() {
    let raw = "id=s;type=idle;msg=ciao;cont=1;bg=3;src=clear;why=other;path=/p;tool=T";
    let details = FakeHooks::default().normalize(HookEventKind::Finished, raw);
    let p = build_payload("finished", "fake", Some("t1"), details);
    assert_eq!((p.event.as_str(), p.agent.as_str(), p.tab_id.as_deref()), ("finished", "fake", Some("t1")));
    assert_eq!((p.agent_session_id.as_deref(), p.notification_type.as_deref()), (Some("s"), Some("idle")));
    assert_eq!((p.message.as_deref(), p.stop_hook_active, p.background_work), (Some("ciao"), Some(true), Some(3)));
    assert_eq!((p.source.as_deref(), p.reason.as_deref()), (Some("clear"), Some("other")));
    assert_eq!((p.transcript_path.as_deref(), p.tool_name.as_deref(), p.token), (Some("/p"), Some("T"), None));
    // Id tab non valido: non finisce nel payload.
    assert_eq!(build_payload("test", "fake", Some("a b"), HookEvent::default()).tab_id, None);
}

/// Protezione: nessun test puo' raggiungere l'app reale, nemmeno per
/// sbaglio (l'asserzione scatta prima della connessione).
#[test]
#[should_panic(expected = "tests must never connect to the real app")]
fn tests_cannot_connect_to_the_real_app() {
    let _ = send_to(DEFAULT_PORT, &sample("test"));
}

#[test]
#[should_panic(expected = "tests must never connect to the real app")]
fn tests_cannot_call_the_real_app() {
    let _ = rpc_call(DEFAULT_PORT, "{}", Duration::from_millis(100));
}

#[test]
#[should_panic(expected = "tests must never connect to the real app")]
fn tests_cannot_connect_to_the_port_of_the_enclosing_terminal() {
    guard_real_app(41999, Some(" 41999 "));
}

#[test]
fn the_guard_lets_other_ports_through() {
    guard_real_app(41999, None);
    guard_real_app(41999, Some("41998"));
    guard_real_app(41999, Some("non una porta"));
}

#[test]
fn port_resolution_falls_back_to_the_default() {
    assert_eq!(parse_port(None), DEFAULT_PORT);
    for bad in ["", "   ", "abc", "0", "70000", "-1", "17421x"] {
        assert_eq!(parse_port(Some(bad)), DEFAULT_PORT, "{bad:?}");
    }
    assert_eq!(parse_port(Some(" 17500 ")), 17500);
    assert_eq!(parse_port(Some("65535")), 65535);
    assert_eq!(parse_port(Some("1")), 1);
}

#[test]
fn only_registered_agents_with_the_contract_have_hooks() {
    assert!(hook_events_of(crate::agents::DEFAULT_WORKER_AGENT).is_some());
    assert!(hook_events_of("nope").is_none());
}

#[test]
fn notify_prints_block_decision_in_the_agent_format() {
    let (port, h) = fake_listener(Some(r#"{"block":"x"}"#), Duration::ZERO);
    let mut out = Vec::new();
    notify_via(port, &finished_with_token(), Some(&FakeHooks::default()), Duration::from_secs(2), &mut out).unwrap();
    h.join().unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "STOP[x]\n");
}

#[test]
fn notify_target_prints_the_block_of_a_finished_event_end_to_end() {
    let (port, h) = fake_listener(Some(r#"{"block":"go on"}"#), Duration::ZERO);
    let hooks = FakeHooks::default();
    let target = NotifyTarget { tab_id: Some("tab-1"), token: Some("tok"), port };
    let mut out = Vec::new();
    notify_target("finished", "fake", &target, || Some("cont=0".into()), Some(&hooks), &mut out).unwrap();
    h.join().unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "STOP[go on]\n");
    assert_eq!(hooks.kinds(), vec![HookEventKind::Finished]);
}

#[test]
fn notify_prints_nothing_without_a_contract() {
    let (port, h) = fake_listener(Some(r#"{"block":"x"}"#), Duration::ZERO);
    let mut out = Vec::new();
    notify_via(port, &finished_with_token(), None, Duration::from_secs(2), &mut out).unwrap();
    h.join().unwrap();
    assert!(out.is_empty());
}

#[test]
fn notify_prints_nothing_without_block_or_on_timeout() {
    let hooks = FakeHooks::default();
    for reply in [Some("{}"), Some("garbage"), Some(r#"{"block":null}"#)] {
        let (port, h) = fake_listener(reply, Duration::ZERO);
        let mut out = Vec::new();
        notify_via(port, &finished_with_token(), Some(&hooks), Duration::from_secs(2), &mut out).unwrap();
        h.join().unwrap();
        assert!(out.is_empty(), "{reply:?}");
    }
    // Nessuna risposta entro il timeout: Ok e stdout vuoto.
    let (port, h) = fake_listener(None, Duration::from_millis(600));
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    notify_via(port, &finished_with_token(), Some(&hooks), Duration::from_millis(200), &mut out).unwrap();
    assert!(t0.elapsed() < Duration::from_millis(550));
    h.join().unwrap();
    assert!(out.is_empty());
    // Porta chiusa: errore di invio come prima.
    let closed = notify_via(1, &finished_with_token(), Some(&hooks), Duration::from_millis(100), &mut Vec::new());
    assert!(closed.is_err());
}

#[test]
fn rpc_call_errors() {
    assert!(rpc_call(1, "{}", Duration::from_secs(1)).is_err()); // porta chiusa
                                                                 // Server che non risponde mai: timeout.
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let _keep = std::thread::spawn(move || {
        let (_s, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(1500));
    });
    let e = rpc_call(port, "{}", Duration::from_millis(300)).unwrap_err();
    assert!(e.contains("timeout"), "{e}");
}

#[test]
fn rpc_call_reports_a_connection_closed_without_an_answer() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        drop(s);
    });
    let e = rpc_call(port, "{}", Duration::from_secs(2)).unwrap_err();
    server.join().unwrap();
    assert!(e.contains("closed") || e.contains("failed"), "{e}");
}
