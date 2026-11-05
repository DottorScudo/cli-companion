use super::env::{apply_env, env_vars, merge_wslenv};
use super::exit::run_within;
use super::io::{decode_incremental, spawn_writer};
use super::registry::{PtySession, Registry, Reservation, ReserveError, SharedRegistry, MAX_SESSIONS};
use super::spawn::{
    build_command, check_start_dir, is_powershell, open_pty, pty_size, session_command, start_dir,
    POWERSHELL_TITLE_HOOK,
};
use super::staged_env::{StagedEnv, MAX_STAGED};
use crate::ipc::{PORT_ENV, SESSION_ENV, TOKEN_ENV};
use crate::platform::{Target, TargetKind};
use portable_pty::CommandBuilder;
use std::io::Write;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn new_registry() -> SharedRegistry {
    Arc::new(Mutex::new(Registry::default()))
}

fn wsl(distro: &str) -> Target {
    Target::Wsl { distro: distro.to_string() }
}

fn argv(cmd: &CommandBuilder) -> Vec<String> {
    cmd.get_argv().iter().map(|a| a.to_string_lossy().into_owned()).collect()
}

fn env_value(vars: &[(String, String)], key: &str) -> Option<String> {
    vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Sessione vera su un PTY vero (`cmd` / `sh`), per esercitare la tabella
/// con sessioni vive.
fn live_session() -> PtySession {
    let program = if cfg!(windows) { "cmd.exe" } else { "sh" };
    let opened = open_pty(CommandBuilder::new(program), pty_size(80, 24)).expect("pty di prova");
    PtySession {
        master: Arc::new(Mutex::new(opened.master)),
        resize_seq: Arc::new(AtomicU64::new(0)),
        writer: spawn_writer(opened.writer),
        child: Mutex::new(opened.child),
    }
}

fn reserve(registry: &SharedRegistry, tab: &str) -> Result<Reservation, ReserveError> {
    Reservation::new(registry, tab, "ws-1")
}

// ---------------------------------------------------------------------------
// Decodifica dell'output
// ---------------------------------------------------------------------------

#[test]
fn decode_ascii_passthrough() {
    let mut pending = b"ciao".to_vec();
    assert_eq!(decode_incremental(&mut pending), "ciao");
    assert!(pending.is_empty());
}

#[test]
fn decode_split_multibyte_waits_for_rest() {
    // "à" = C3 A0 spezzata tra due chunk: il primo non emette nulla.
    let mut pending = vec![0xC3];
    assert_eq!(decode_incremental(&mut pending), "");
    assert_eq!(pending, vec![0xC3]);
    pending.push(0xA0);
    assert_eq!(decode_incremental(&mut pending), "à");
    assert!(pending.is_empty());
}

#[test]
fn decode_invalid_byte_emits_replacement() {
    let mut pending = vec![0xFF];
    assert_eq!(decode_incremental(&mut pending), "�");
    assert!(pending.is_empty());
}

// ---------------------------------------------------------------------------
// Ambiente di sessione
// ---------------------------------------------------------------------------

#[test]
fn wslenv_merge_appends_without_duplicates() {
    let v = ["CLI_COMPANION_SESSION"];
    assert_eq!(merge_wslenv(None, &v), "CLI_COMPANION_SESSION/u");
    assert_eq!(merge_wslenv(Some(""), &v), "CLI_COMPANION_SESSION/u");
    assert_eq!(merge_wslenv(Some("PATH/l"), &v), "PATH/l:CLI_COMPANION_SESSION/u");
    assert_eq!(merge_wslenv(Some("A:B/p:"), &v), "A:B/p:CLI_COMPANION_SESSION/u");
    // Gia' presente (con o senza flag): invariato.
    assert_eq!(merge_wslenv(Some("A:CLI_COMPANION_SESSION/u"), &v), "A:CLI_COMPANION_SESSION/u");
    assert_eq!(merge_wslenv(Some("CLI_COMPANION_SESSION"), &v), "CLI_COMPANION_SESSION");
    // Nome che contiene solo la variabile come prefisso: non conta.
    assert_eq!(merge_wslenv(Some("CLI_COMPANION_SESSION_X"), &v), "CLI_COMPANION_SESSION_X:CLI_COMPANION_SESSION/u");
}

#[test]
fn wslenv_merge_handles_several_vars() {
    let v = ["S", "T"];
    assert_eq!(merge_wslenv(None, &v), "S/u:T/u");
    assert_eq!(merge_wslenv(Some("A/p"), &v), "A/p:S/u:T/u");
    // Una gia' presente: si aggiunge solo l'altra.
    assert_eq!(merge_wslenv(Some("T/u"), &v), "T/u:S/u");
    assert_eq!(merge_wslenv(Some("S:T/u"), &v), "S:T/u");
}

#[test]
fn session_env_is_set_and_wsl_adds_wslenv() {
    let windows = env_vars("tab-1", "tok", Some(17500), TargetKind::Windows, Some("PATH/l"), &[]);
    assert_eq!(env_value(&windows, SESSION_ENV).as_deref(), Some("tab-1"));
    assert_eq!(env_value(&windows, TOKEN_ENV).as_deref(), Some("tok"));
    assert_eq!(env_value(&windows, PORT_ENV).as_deref(), Some("17500"));
    assert_eq!(env_value(&windows, "WSLENV"), None);

    let wsl = env_vars("tab-1", "tok", Some(17500), TargetKind::Wsl, Some("PATH/l"), &[]);
    assert_eq!(env_value(&wsl, SESSION_ENV).as_deref(), Some("tab-1"));
    assert_eq!(env_value(&wsl, TOKEN_ENV).as_deref(), Some("tok"));
    assert_eq!(env_value(&wsl, PORT_ENV).as_deref(), Some("17500"));
    assert_eq!(
        env_value(&wsl, "WSLENV").as_deref(),
        Some("PATH/l:CLI_COMPANION_SESSION/u:CLI_COMPANION_TOKEN/u:CLI_COMPANION_PORT/u")
    );
}

#[test]
fn port_is_set_and_forwarded_only_when_the_listener_is_up() {
    let wsl = env_vars("tab-1", "tok", None, TargetKind::Wsl, Some("PATH/l"), &[]);
    assert_eq!(env_value(&wsl, PORT_ENV), None);
    assert_eq!(env_value(&wsl, "WSLENV").as_deref(), Some("PATH/l:CLI_COMPANION_SESSION/u:CLI_COMPANION_TOKEN/u"));
    let windows = env_vars("tab-1", "tok", None, TargetKind::Windows, None, &[]);
    assert_eq!(env_value(&windows, PORT_ENV), None);
}

#[test]
fn color_env_declares_256_and_truecolor() {
    let vars = env_vars("tab-1", "tok", None, TargetKind::Windows, None, &[]);
    assert_eq!(env_value(&vars, "TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(env_value(&vars, "COLORTERM").as_deref(), Some("truecolor"));
}

#[test]
fn apply_env_uses_the_given_secret() {
    let mut cmd = CommandBuilder::new("dummy");
    apply_env(&mut cmd, "tab-1", "secret-1", TargetKind::Windows, &[]);
    assert_eq!(cmd.get_env(SESSION_ENV), Some(std::ffi::OsStr::new("tab-1")));
    assert_eq!(cmd.get_env(TOKEN_ENV), Some(std::ffi::OsStr::new("secret-1")));
    // Nei test il listener non e' avviato: nessuna porta (e quella ereditata e' rimossa).
    assert_eq!(cmd.get_env(PORT_ENV), None);
    assert_eq!(cmd.get_env("TERM"), Some(std::ffi::OsStr::new("xterm-256color")));
}

// ---------------------------------------------------------------------------
// Costruzione del comando
// ---------------------------------------------------------------------------

#[test]
fn powershell_detection_matches_names_paths_and_case() {
    for yes in [
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
        "POWERSHELL.EXE",
        "  pwsh  ",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        r"C:\Program Files\PowerShell\7\pwsh.exe",
    ] {
        assert!(is_powershell(yes), "{yes}");
    }
    for no in ["", "cmd", "cmd.exe", "bash", "zsh", "nu", "powershell.exe.bak"] {
        assert!(!is_powershell(no), "{no}");
    }
}

#[test]
fn title_hook_emits_osc_and_chains_prompt() {
    assert!(POWERSHELL_TITLE_HOOK.contains("function global:prompt"));
    assert!(POWERSHELL_TITLE_HOOK.contains("[char]27)]0;"));
    assert!(POWERSHELL_TITLE_HOOK.contains("RawUI.WindowTitle"));
    assert!(POWERSHELL_TITLE_HOOK.contains("Get-Command prompt"));
}

/// Esercita il vero percorso di consegna dell'hook (argv di CreateProcess
/// -> powershell.exe) e ne verifica il comportamento end-to-end.
#[test]
#[cfg(windows)]
fn title_hook_roundtrip_through_powershell() {
    fn run(prefix: &str, suffix: &str) -> String {
        let cmd = format!("{prefix}{POWERSHELL_TITLE_HOOK}{suffix}");
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-Command", &cmd])
            .output()
            .expect("powershell.exe non avviabile");
        assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    // Default: OSC 0 con fallback + prompt ancora funzionante.
    let out = run("", "; 1+1; prompt");
    assert!(out.contains('2'), "comandi rotti? output: {out:?}");
    assert!(out.contains("\u{1b}]0;"), "OSC 0 mancante: {out:?}");
    assert!(out.contains('\u{7}'), "BEL mancante: {out:?}");

    // Mirror: un titolo impostato via API Win32 (come fanno gli agenti)
    // viene riemesso come OSC al prompt successivo. Titolo unico per
    // esecuzione: la console e' condivisa con chi lancia i test e il titolo
    // resta impostato, quindi al giro dopo sarebbe gia' quello "iniziale".
    let title = format!(
        "AGENTE-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    );
    let out = run("", &format!("; $Host.UI.RawUI.WindowTitle = '{title}'; prompt"));
    assert!(out.contains(&format!("]0;{title}\x07")), "mirror mancante: {out:?}");

    // Chaining: un prompt definito dal profilo viene preservato.
    let out = run("function prompt { 'CUSTOM> ' }; ", "; prompt");
    assert!(out.contains("CUSTOM> "), "prompt profilo perso: {out:?}");
    assert!(out.contains("\u{1b}]0;"), "OSC 0 mancante con profilo: {out:?}");
}

#[test]
fn windows_command_defaults_to_powershell_with_title_hook() {
    let cmd = build_command(&Target::Windows, r"C:\work", None);
    assert_eq!(argv(&cmd), ["powershell.exe", "-NoLogo", "-NoExit", "-Command", POWERSHELL_TITLE_HOOK]);
    assert_eq!(cmd.get_cwd().map(|d| d.to_string_lossy().into_owned()).as_deref(), Some(r"C:\work"));
    // Una shell vuota equivale a nessuna shell.
    assert_eq!(argv(&build_command(&Target::Windows, r"C:\work", Some(""))), argv(&cmd));
}

#[test]
fn windows_command_passes_other_shells_through_without_the_hook() {
    let cmd = build_command(&Target::Windows, r"C:\work", Some("cmd.exe"));
    assert_eq!(argv(&cmd), ["cmd.exe"]);
    assert_eq!(cmd.get_cwd().map(|d| d.to_string_lossy().into_owned()).as_deref(), Some(r"C:\work"));

    let pwsh = build_command(&Target::Windows, r"C:\work", Some(r"C:\Program Files\PowerShell\7\pwsh.exe"));
    assert_eq!(argv(&pwsh)[1..4], ["-NoLogo", "-NoExit", "-Command"]);
}

#[test]
fn wsl_command_enters_the_distro_at_the_directory() {
    let cmd = build_command(&wsl("Ubuntu-24.04"), "/home/me/app", None);
    assert_eq!(argv(&cmd), ["wsl.exe", "-d", "Ubuntu-24.04", "--cd", "/home/me/app"]);
    // La cartella la imposta `--cd`, non il cwd del processo Windows.
    assert!(cmd.get_cwd().is_none());
}

#[test]
fn wsl_command_execs_the_requested_shell() {
    let cmd = build_command(&wsl("Ubuntu"), "/srv", Some("/bin/zsh"));
    assert_eq!(argv(&cmd), ["wsl.exe", "-d", "Ubuntu", "--cd", "/srv", "-e", "/bin/zsh"]);
    assert_eq!(argv(&build_command(&wsl("Ubuntu"), "/srv", Some(""))), ["wsl.exe", "-d", "Ubuntu", "--cd", "/srv"]);
}

#[test]
fn session_command_adds_the_environment_of_its_target() {
    let windows = session_command(&Target::Windows, r"C:\work", None, "tab-1", "secret-1", &[]);
    assert_eq!(windows.get_env(SESSION_ENV), Some(std::ffi::OsStr::new("tab-1")));
    assert_eq!(windows.get_env(TOKEN_ENV), Some(std::ffi::OsStr::new("secret-1")));
    assert_eq!(windows.get_env("COLORTERM"), Some(std::ffi::OsStr::new("truecolor")));

    let wsl = session_command(&wsl("Ubuntu"), "/srv", None, "tab-2", "secret-2", &[]);
    assert_eq!(wsl.get_env(SESSION_ENV), Some(std::ffi::OsStr::new("tab-2")));
    assert_eq!(wsl.get_env(TOKEN_ENV), Some(std::ffi::OsStr::new("secret-2")));
    let wslenv = wsl.get_env("WSLENV").unwrap().to_string_lossy().into_owned();
    assert!(wslenv.split(':').any(|e| e == "CLI_COMPANION_SESSION/u"), "{wslenv}");
    assert!(wslenv.split(':').any(|e| e == "CLI_COMPANION_TOKEN/u"), "{wslenv}");
}

#[test]
fn start_dir_prefers_a_non_blank_request() {
    assert_eq!(start_dir(Some("  /tmp/x  "), "/ws"), "/tmp/x");
    assert_eq!(start_dir(Some("   "), "/ws"), "/ws");
    assert_eq!(start_dir(Some(""), "/ws"), "/ws");
    assert_eq!(start_dir(None, "/ws"), "/ws");
}

#[test]
fn windows_start_dir_must_be_an_existing_directory() {
    let dir = std::env::temp_dir();
    assert_eq!(check_start_dir(&Target::Windows, &dir.to_string_lossy()), Ok(()));

    let missing = dir.join("pty-hardening-cartella-inesistente");
    let err = check_start_dir(&Target::Windows, &missing.to_string_lossy()).unwrap_err();
    assert!(err.starts_with("cartella non trovata"), "{err}");

    let file = std::env::current_exe().unwrap();
    let err = check_start_dir(&Target::Windows, &file.to_string_lossy()).unwrap_err();
    assert!(err.starts_with("non è una cartella"), "{err}");
}

#[test]
fn size_is_clamped() {
    let s = pty_size(0, 1000);
    assert_eq!((s.cols, s.rows), (2, 300));
    let s = pty_size(80, 24);
    assert_eq!((s.cols, s.rows), (80, 24));
}

// ---------------------------------------------------------------------------
// Flusso di scrittura
// ---------------------------------------------------------------------------

#[test]
fn writer_queue_preserves_order_and_stops_on_close() {
    struct Sink(std::sync::mpsc::Sender<Vec<u8>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.send(b.to_vec()).unwrap();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let tx = spawn_writer(Box::new(Sink(out_tx)));
    for i in 0..50u8 {
        tx.send(vec![i]).unwrap();
    }
    drop(tx);
    // Chiuso il canale il thread esce e chiude anche `out_tx`.
    let got: Vec<u8> = out_rx.iter().flatten().collect();
    assert_eq!(got, (0..50u8).collect::<Vec<_>>());
}

// ---------------------------------------------------------------------------
// Prenotazione degli slot
// ---------------------------------------------------------------------------

#[test]
fn reservation_is_refused_when_all_slots_are_taken() {
    let registry = new_registry();
    let held: Vec<Reservation> = (0..MAX_SESSIONS).map(|i| reserve(&registry, &format!("tab-{i}")).unwrap()).collect();

    assert_eq!(reserve(&registry, "one-too-many").err(), Some(ReserveError::Full));
    assert_eq!(ReserveError::Full.to_string(), "troppe sessioni aperte");

    // Liberato uno slot, il successivo passa.
    let mut held = held;
    held.pop();
    assert!(reserve(&registry, "one-too-many").is_ok());
}

#[test]
fn reservation_is_refused_for_a_tab_that_already_has_a_session() {
    let registry = new_registry();
    let _first = reserve(&registry, "tab-1").unwrap();

    assert_eq!(reserve(&registry, "tab-1").err(), Some(ReserveError::DuplicateTab));
    // Anche da un altro workspace: il tab e' globale.
    assert_eq!(Reservation::new(&registry, "tab-1", "ws-2").err(), Some(ReserveError::DuplicateTab));
    assert!(reserve(&registry, "tab-2").is_ok());
}

#[test]
fn concurrent_reservations_never_exceed_the_limit_nor_repeat_a_tab() {
    let registry = new_registry();
    let granted = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..MAX_SESSIONS * 2)
        .map(|i| {
            let (registry, granted) = (registry.clone(), granted.clone());
            std::thread::spawn(move || {
                // Meta' dei thread gareggia sullo stesso tab.
                let tab = format!("tab-{}", i % MAX_SESSIONS);
                reserve(&registry, &tab).ok().inspect(|_| {
                    granted.fetch_add(1, Ordering::SeqCst);
                })
            })
        })
        .collect();
    let kept: Vec<_> = handles.into_iter().filter_map(|h| h.join().unwrap()).collect();

    assert_eq!(granted.load(Ordering::SeqCst), MAX_SESSIONS);
    assert_eq!(kept.len(), MAX_SESSIONS);
}

#[test]
fn dropping_an_unconfirmed_reservation_frees_slot_and_tab() {
    let registry = new_registry();
    drop(reserve(&registry, "tab-1").unwrap());

    assert!(reserve(&registry, "tab-1").is_ok());
}

/// Lo stesso ordine di `open_session`: prenota, poi avvia. Un comando che non
/// parte non deve lasciare lo slot occupato.
#[test]
fn failed_spawn_releases_the_slot() {
    fn spawn_missing_program(registry: &SharedRegistry, tab: &str) -> Result<u32, String> {
        let reservation = reserve(registry, tab).map_err(|e| e.to_string())?;
        let cmd = CommandBuilder::new("pty-hardening-programma-inesistente");
        let _opened = open_pty(cmd, pty_size(80, 24))?;
        Ok(reservation.id())
    }

    let registry = new_registry();
    for _ in 0..MAX_SESSIONS + 1 {
        let err = spawn_missing_program(&registry, "tab-1").unwrap_err();
        assert!(err.starts_with("spawn:"), "{err}");
    }
    assert!(reserve(&registry, "tab-1").is_ok());
}

#[test]
fn a_reserved_slot_has_no_live_session_yet() {
    let registry = new_registry();
    let reservation = reserve(&registry, "tab-1").unwrap();
    let mut table = registry.lock().unwrap();

    assert_eq!(table.workspace_of_tab("tab-1"), None);
    assert!(table.writer(reservation.id()).is_none());
    assert!(table.writer_of_tab("tab-1").is_none());
    assert!(table.session(reservation.id()).is_none());
    // `pty_close` o il reader di un'altra sessione non possono rubare la prenotazione.
    assert!(table.take_session(reservation.id()).is_none());
    drop(table);
    assert_eq!(reserve(&registry, "tab-1").err(), Some(ReserveError::DuplicateTab));
}

// ---------------------------------------------------------------------------
// Sessioni vive (PTY reali)
// ---------------------------------------------------------------------------

#[test]
fn committed_session_is_found_by_tab_deterministically() {
    let registry = new_registry();
    let a = Reservation::new(&registry, "tab-a", "ws-a").unwrap();
    let b = Reservation::new(&registry, "tab-b", "ws-b").unwrap();
    let (id_a, id_b) = (a.commit(live_session()).unwrap(), b.commit(live_session()).unwrap());
    assert_ne!(id_a, id_b);

    let mut table = registry.lock().unwrap();
    for _ in 0..20 {
        assert_eq!(table.workspace_of_tab("tab-a"), Some("ws-a"));
        assert_eq!(table.workspace_of_tab("tab-b"), Some("ws-b"));
    }
    assert_eq!(table.workspace_of_tab("tab-c"), None);
    assert_eq!(table.writer(id_a).map(|(_, tab)| tab).as_deref(), Some("tab-a"));
    assert!(table.writer_of_tab("tab-a").is_some());
    assert!(table.writer_of_tab("tab-c").is_none());

    // Rimossa la sessione, il tab non risolve piu' e puo' riaprirsi.
    let session = table.take_session(id_a).expect("sessione viva");
    assert_eq!(table.workspace_of_tab("tab-a"), None);
    assert!(table.writer_of_tab("tab-a").is_none());
    assert!(table.take_session(id_a).is_none());
    drop(table);
    session.shutdown();
    assert!(reserve(&registry, "tab-a").is_ok());
    registry.lock().unwrap().take_session(id_b).expect("sessione viva").shutdown();
}

#[test]
fn closing_refuses_new_sessions_and_hands_back_the_live_ones() {
    let registry = new_registry();
    let live = reserve(&registry, "tab-live").unwrap().commit(live_session()).unwrap();
    let starting = reserve(&registry, "tab-starting").unwrap();

    let to_close = registry.lock().unwrap().begin_close();
    assert_eq!(to_close.len(), 1);
    assert!(registry.lock().unwrap().session(live).is_none());
    assert_eq!(reserve(&registry, "tab-new").err(), Some(ReserveError::Closing));

    // Una shell che finisce di avviarsi a chiusura iniziata viene terminata.
    assert_eq!(starting.commit(live_session()), Err("app in chiusura".to_string()));
    assert!(reserve(&registry, "tab-starting").is_err());

    let finished = run_within(to_close, Duration::from_secs(10), PtySession::shutdown);
    assert_eq!(finished, 1);
}

// ---------------------------------------------------------------------------
// Chiusura all'uscita
// ---------------------------------------------------------------------------

#[test]
fn run_within_waits_for_all_when_they_are_fast() {
    let done = Arc::new(AtomicUsize::new(0));
    let counter = done.clone();
    let finished = run_within(vec![1, 2, 3], Duration::from_secs(5), move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(finished, 3);
    assert_eq!(done.load(Ordering::SeqCst), 3);
}

#[test]
fn run_within_gives_up_on_a_stuck_item_at_the_deadline() {
    let started = Instant::now();
    let finished = run_within(vec![0u64, 1], Duration::from_millis(200), |ms| {
        if ms == 1 {
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    assert_eq!(finished, 1);
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[test]
fn run_within_handles_no_items() {
    assert_eq!(run_within(Vec::<u8>::new(), Duration::from_millis(50), |_| {}), 0);
}

fn staged(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn staged_env_is_set_after_the_session_variables_on_windows() {
    let extra = staged(&[("OPENCODE_CONFIG_CONTENT", r#"{"a":"b $c"}"#), ("FAKE_AGENT_MODE", "plan")]);
    let vars = env_vars("tab-1", "tok", None, TargetKind::Windows, None, &extra);
    assert_eq!(env_value(&vars, "OPENCODE_CONFIG_CONTENT").as_deref(), Some(r#"{"a":"b $c"}"#));
    assert_eq!(env_value(&vars, "FAKE_AGENT_MODE").as_deref(), Some("plan"));
    assert_eq!(env_value(&vars, SESSION_ENV).as_deref(), Some("tab-1"));
    assert_eq!(env_value(&vars, "WSLENV"), None);
    // Vengono dopo le variabili del core.
    let pos = |k: &str| vars.iter().position(|(n, _)| n == k).unwrap();
    assert!(pos("FAKE_AGENT_MODE") > pos(TOKEN_ENV));
}

#[test]
fn staged_env_names_are_merged_into_wslenv_without_duplicates() {
    let extra = staged(&[("OPENCODE_CONFIG", "/mnt/c/x/cfg.json"), ("FAKE_AGENT_MODE", "plan")]);
    let vars = env_vars("tab-1", "tok", None, TargetKind::Wsl, Some("PATH/l:FAKE_AGENT_MODE/p"), &extra);
    assert_eq!(env_value(&vars, "OPENCODE_CONFIG").as_deref(), Some("/mnt/c/x/cfg.json"));
    assert_eq!(
        env_value(&vars, "WSLENV").as_deref(),
        Some("PATH/l:FAKE_AGENT_MODE/p:CLI_COMPANION_SESSION/u:CLI_COMPANION_TOKEN/u:OPENCODE_CONFIG/u")
    );
}

#[test]
fn reserved_or_malformed_staged_names_are_never_applied() {
    let extra = staged(&[
        ("CLI_COMPANION_TOKEN", "evil"),
        ("WSLENV", "evil"),
        ("TERM", "evil"),
        ("lower", "x"),
        ("OK_VAR", "1"),
    ]);
    let vars = env_vars("tab-1", "tok", None, TargetKind::Wsl, None, &extra);
    assert_eq!(env_value(&vars, TOKEN_ENV).as_deref(), Some("tok"));
    assert_eq!(env_value(&vars, "TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(env_value(&vars, "lower"), None);
    assert_eq!(vars.iter().filter(|(n, _)| n == "WSLENV").count(), 1);
    assert_eq!(env_value(&vars, "WSLENV").as_deref(), Some("CLI_COMPANION_SESSION/u:CLI_COMPANION_TOKEN/u:OK_VAR/u"));
}

#[test]
fn session_command_applies_staged_env_to_the_process() {
    let extra = staged(&[("FAKE_AGENT_MODE", "bypass")]);
    let windows = session_command(&Target::Windows, r"C:\work", None, "tab-1", "secret-1", &extra);
    assert_eq!(windows.get_env("FAKE_AGENT_MODE"), Some(std::ffi::OsStr::new("bypass")));
    let wsl = session_command(&wsl("Ubuntu"), "/srv", None, "tab-2", "secret-2", &extra);
    assert_eq!(wsl.get_env("FAKE_AGENT_MODE"), Some(std::ffi::OsStr::new("bypass")));
    let wslenv = wsl.get_env("WSLENV").unwrap().to_string_lossy().into_owned();
    assert!(wslenv.split(':').any(|e| e == "FAKE_AGENT_MODE/u"), "{wslenv}");
}

#[test]
fn staged_env_is_consumed_once_and_discarded_on_close() {
    let store = StagedEnv::default();
    store.stage("tab-1", staged(&[("A", "1")]));
    assert_eq!(store.take("tab-1"), staged(&[("A", "1")]));
    assert!(store.take("tab-1").is_empty());
    assert!(store.take("never-staged").is_empty());

    store.stage("tab-2", staged(&[("A", "2")]));
    store.stage("tab-2", staged(&[("A", "3")]));
    assert_eq!(store.len(), 1);
    store.discard("tab-2");
    assert!(store.take("tab-2").is_empty());
}

#[test]
fn staged_env_is_bounded() {
    let store = StagedEnv::default();
    for i in 0..(MAX_STAGED + 10) {
        store.stage(&format!("tab-{i}"), staged(&[("A", "1")]));
    }
    assert_eq!(store.len(), MAX_STAGED);
    // Le piu' vecchie hanno ceduto il posto.
    assert!(store.take("tab-0").is_empty());
    assert!(!store.take(&format!("tab-{}", MAX_STAGED + 9)).is_empty());
}
