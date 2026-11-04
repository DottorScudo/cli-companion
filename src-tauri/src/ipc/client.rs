//! Lato client del canale: `cli-companion --notify` (gli hook degli agenti) e
//! le chiamate RPC del server MCP. Come leggere lo stdin di un hook lo decide
//! il contratto `HookEvents` dell'agente; qui c'e' solo il trasporto.

use super::wire::{
    build_line, hook_kind, is_known_event, is_valid_tab_id, notification_type_of, NotifyPayload, NotifyReply,
    DEFAULT_PORT, EVENT_FINISHED, EVENT_PERMISSION_REQUEST, PORT_ENV, RPC_RESPONSE_MAX_BYTES, SESSION_ENV, TOKEN_ENV,
    WIRE_MAX_BYTES,
};
use crate::agents::{HookEvent, HookEvents};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

#[cfg(test)]
mod tests;

/// Attesa massima della connessione al listener (client `--notify` e RPC).
const CONNECT_TIMEOUT: Duration = Duration::from_millis(1500);
/// Tetto dello stdin degli hook (puo' contenere l'ultimo messaggio dell'agente).
const STDIN_MAX_BYTES: u64 = 1024 * 1024;
/// Attesa massima della riga di risposta a `finished` (l'hook ha 10 s).
const REPLY_TIMEOUT: Duration = Duration::from_secs(4);

/// Interpreta il valore di `PORT_ENV`: un u16 non nullo, altrimenti la porta
/// preferita.
pub(crate) fn parse_port(raw: Option<&str>) -> u16 {
    raw.and_then(|r| r.trim().parse::<u16>().ok()).filter(|p| *p != 0).unwrap_or(DEFAULT_PORT)
}

/// Porta su cui parlare con l'app: quella che il terminale dichiara in
/// `PORT_ENV` (l'app puo' essere in ascolto su una porta di ripiego).
pub fn app_port() -> u16 {
    parse_port(std::env::var(PORT_ENV).ok().as_deref())
}

/// Solo test: panico se `port` e' quella dell'app reale, cioe' la porta
/// preferita o quella che il terminale in cui gira la suite dichiara in
/// `PORT_ENV` (`env`).
#[cfg(test)]
fn guard_real_app(port: u16, env: Option<&str>) {
    assert_ne!(port, DEFAULT_PORT, "tests must never connect to the real app (default port)");
    let from_env = env.and_then(|v| v.trim().parse::<u16>().ok());
    assert_ne!(Some(port), from_env, "tests must never connect to the real app (port from the environment)");
}

/// Connessione TCP al listener locale su `port`: unico punto da cui notifiche
/// e RPC raggiungono l'app.
fn connect_local(port: u16) -> std::io::Result<TcpStream> {
    // La suite gira anche dal terminale di un worker: un test che arrivasse
    // all'app reale le parlerebbe a nome del worker (es. un `session-end` che
    // gli fa fallire il task). I test usano porte effimere.
    #[cfg(test)]
    guard_real_app(port, std::env::var(PORT_ENV).ok().as_deref());
    TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), CONNECT_TIMEOUT)
}

/// Connette al listener e spedisce la riga del payload.
fn connect_and_send(port: u16, payload: &NotifyPayload) -> Result<TcpStream, String> {
    let line = build_line(payload)?;
    let mut stream = connect_local(port).map_err(|_| "app non in ascolto (avvia cli-companion)".to_string())?;
    stream.write_all(format!("{line}\n").as_bytes()).map_err(|e| format!("invio fallito: {e}"))?;
    stream.flush().map_err(|e| format!("invio fallito: {e}"))?;
    Ok(stream)
}

/// Spedisce un payload al listener locale senza attendere la risposta.
pub(super) fn send_to(port: u16, payload: &NotifyPayload) -> Result<(), String> {
    connect_and_send(port, payload).map(|_| ())
}

/// Spedisce il payload e attende la riga di risposta (al massimo `timeout`).
/// Un errore di invio e' un errore vero; timeout, chiusura o risposta
/// illeggibile danno `Ok(None)`: l'hook si comporta come senza risposta.
pub(super) fn send_for_reply(
    port: u16,
    payload: &NotifyPayload,
    timeout: Duration,
) -> Result<Option<NotifyReply>, String> {
    let stream = connect_and_send(port, payload)?;
    if stream.set_read_timeout(Some(timeout)).is_err() {
        return Ok(None);
    }
    let mut line = String::new();
    let read = BufReader::new(stream.take(WIRE_MAX_BYTES)).read_line(&mut line);
    if read.is_err() {
        return Ok(None);
    }
    Ok(serde_json::from_str(line.trim()).ok())
}

/// Corpo di `--notify` dopo la costruzione del payload. Per `finished` con
/// token attende la risposta del listener e, se contiene `block`, scrive su
/// `out` la decisione di blocco nel formato dell'agente (`hooks`); altrimenti
/// non scrive nulla.
fn notify_via(
    port: u16,
    payload: &NotifyPayload,
    hooks: Option<&dyn HookEvents>,
    timeout: Duration,
    out: &mut dyn Write,
) -> Result<(), String> {
    if payload.event != EVENT_FINISHED || payload.token.is_none() {
        return send_to(port, payload);
    }
    let block = send_for_reply(port, payload, timeout)?.and_then(|r| r.block);
    if let (Some(reason), Some(hooks)) = (block, hooks) {
        let _ = writeln!(out, "{}", hooks.stop_block_output(&reason));
        let _ = out.flush();
    }
    Ok(())
}

/// Legge stdin (tetto 1 MB) solo se non e' un terminale; errori = nessun dato.
fn read_stdin_capped() -> Option<String> {
    use std::io::IsTerminal;
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return None;
    }
    let mut buf = Vec::new();
    stdin.lock().take(STDIN_MAX_BYTES).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Destinatario di `--notify`: tab e token del processo, porta del listener.
struct NotifyTarget<'a> {
    tab_id: Option<&'a str>,
    token: Option<&'a str>,
    port: u16,
}

/// Contratto hook dell'agente `agent`, se l'agente e' noto e lo dichiara.
fn hook_events_of(agent: &str) -> Option<&'static dyn HookEvents> {
    crate::agents::registry::find(agent).ok().and_then(|i| i.capabilities().hook_events)
}

/// Esito di `--notify`: Ok se l'evento e' stato ignorato o inviato, Err solo
/// se l'invio di un evento da inviare fallisce. Legge solo l'ambiente reale
/// e fissa la porta vera; la logica sta in `notify_target`.
pub fn run_notify(event: &str, agent: &str) -> Result<(), String> {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (tab_id, token) = (env(SESSION_ENV), env(TOKEN_ENV));
    let target = NotifyTarget { tab_id: tab_id.as_deref(), token: token.as_deref(), port: app_port() };
    notify_target(event, agent, &target, read_stdin_capped, hook_events_of(agent), &mut std::io::stdout())
}

/// Corpo di `--notify` con ambiente, stdin, contratto dell'agente e stdout
/// espliciti. Senza contratto (agente sconosciuto o che non lo dichiara)
/// l'evento parte senza dettagli e non c'e' mai un output di blocco.
fn notify_target(
    event: &str,
    agent: &str,
    target: &NotifyTarget,
    read_stdin: impl FnOnce() -> Option<String>,
    hooks: Option<&dyn HookEvents>,
    out: &mut dyn Write,
) -> Result<(), String> {
    if !should_send(event, target.tab_id) {
        return Ok(());
    }
    // La stdin si legge solo dopo la decisione: un evento ignorato non deve
    // restare appeso su una pipe che nessuno chiude.
    let raw = read_stdin();
    let details = match (hook_kind(event), raw.as_deref(), hooks) {
        (Some(kind), Some(raw), Some(hooks)) => hooks.normalize(kind, raw),
        _ => HookEvent::default(),
    };
    let mut payload = build_payload(event, agent, target.tab_id, details);
    payload.token = target.token.map(str::to_string);
    notify_via(target.port, &payload, hooks, REPLY_TIMEOUT, out)
}

/// Decide se `--notify` deve spedire l'evento. Gli eventi sconosciuti vanno
/// ignorati in silenzio (exit 0): i vecchi plugin Muse ancora installati
/// continuano a chiamare `--notify stop|session-end`. Gli eventi degli agenti
/// senza id tab vengono da sessioni fuori da cli-companion: ignorati di
/// proposito. Solo `test` passa sempre. Solo un invio fallito di un evento
/// da inviare e' un vero errore.
fn should_send(event: &str, tab_id: Option<&str>) -> bool {
    if !is_known_event(event) {
        return false;
    }
    event == "test" || tab_id.is_some_and(is_valid_tab_id)
}

/// Testo di `needs-input` per un permesso richiesto.
fn permission_text(tool: &str, summary: Option<&str>) -> String {
    match summary {
        Some(s) => format!("Permesso richiesto: {tool} \u{2014} {s}"),
        None => format!("Permesso richiesto: {tool}"),
    }
}

/// Costruisce il payload da evento, agente, id tab e dettagli dell'agente.
pub(super) fn build_payload(event: &str, agent: &str, tab_id: Option<&str>, details: HookEvent) -> NotifyPayload {
    let message = if event == EVENT_PERMISSION_REQUEST {
        details.tool_name.as_deref().map(|t| permission_text(t, details.tool_summary.as_deref()))
    } else {
        details.message
    };
    NotifyPayload {
        event: event.to_string(),
        agent: agent.to_string(),
        tab_id: tab_id.filter(|t| is_valid_tab_id(t)).map(str::to_string),
        agent_session_id: details.agent_session_id,
        notification_type: notification_type_of(details.wait),
        message,
        token: None,
        stop_hook_active: details.already_continuing,
        background_work: details.background_work,
        source: details.start_source,
        reason: details.end_reason,
        transcript_path: details.transcript_path,
        tool_name: details.tool_name,
    }
}

/// Client RPC: spedisce `line` (una riga JSON, `\n` aggiunto qui) al listener
/// su `port` e ritorna la riga di risposta. `timeout` e' l'attesa massima
/// della risposta.
pub fn rpc_call(port: u16, line: &str, timeout: Duration) -> Result<String, String> {
    let mut stream =
        connect_local(port).map_err(|_| "the cli-companion app is not listening (start it)".to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(10))))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(format!("{}\n", line.trim_end_matches(['\r', '\n'])).as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|e| format!("send failed: {e}"))?;
    let mut resp = String::new();
    BufReader::new(stream.take(RPC_RESPONSE_MAX_BYTES)).read_line(&mut resp).map_err(|e| match e.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => "timeout waiting for the response".to_string(),
        _ => format!("reading the response failed: {e}"),
    })?;
    if resp.trim().is_empty() {
        return Err("the app closed the connection without answering".to_string());
    }
    Ok(resp.trim_end().to_string())
}
