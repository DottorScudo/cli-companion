//! Lato server del canale: listener TCP locale, un thread per connessione (con
//! un tetto), smistamento di notifiche e RPC all'`IpcHandler` del core e
//! inoltro degli eventi ammessi al frontend.

use super::gate::{forwards_to_frontend, NotifyGate};
use super::wire::{
    read_wire, NotifyPayload, NotifyReply, RpcRequest, RpcResponse, Wire, DEFAULT_PORT, PORT_ENV, SESSION_ENV,
    TOKEN_ENV,
};
use serde::Serialize;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(test)]
mod tests;

/// Nome evento Tauri verso il frontend.
const FRONTEND_EVENT: &str = "agent-event";
/// Connessioni servite contemporaneamente: oltre, la connessione si scarta.
const MAX_CONNECTIONS: usize = 32;

/// Quello che il core offre al canale: l'autenticazione delle sessioni, le RPC
/// e la reazione agli eventi hook. Il canale non sa cosa ne fa il core.
pub trait IpcHandler: Send + Sync {
    /// Token che le sessioni ricevono in `TOKEN_ENV`.
    fn session_token(&self) -> &str;
    fn rpc(&self, req: &RpcRequest) -> RpcResponse;
    /// Evento hook gia' ammesso; `Some(motivo)` = lo stop va bloccato.
    fn hook_event(&self, payload: &NotifyPayload) -> Option<String>;
}

/// Stato del listener, per l'interfaccia e per le sessioni.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct IpcStatus {
    /// Porta in ascolto; `None` se il listener non e' partito.
    pub port: Option<u16>,
    /// Porta che si voleva usare.
    pub preferred_port: u16,
    pub error: Option<String>,
}

static STATUS: OnceLock<IpcStatus> = OnceLock::new();

/// Porta in ascolto, se il listener e' partito.
pub fn listening_port() -> Option<u16> {
    STATUS.get().and_then(|s| s.port)
}

#[tauri::command]
pub fn ipc_status() -> IpcStatus {
    STATUS.get().cloned().unwrap_or_else(|| IpcStatus {
        port: None,
        preferred_port: DEFAULT_PORT,
        error: Some("listener non avviato".to_string()),
    })
}

/// Stato gestito da Tauri: l'handler, per il self-test.
pub struct IpcState {
    handler: Arc<dyn IpcHandler>,
}

/// Apre il listener su `preferred`; se non si puo', su una porta libera
/// qualsiasi (sempre 127.0.0.1).
fn bind_listener(preferred: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", preferred)).or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
}

/// Stato da pubblicare per l'esito dell'apertura del listener, loggando
/// l'eventuale ripiego o fallimento.
fn status_of(preferred: u16, bound: &std::io::Result<TcpListener>) -> IpcStatus {
    let port = match bound {
        Ok(l) => l.local_addr().map(|a| a.port()).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    match port {
        Ok(port) => {
            if port != preferred {
                log::warn!("porta {preferred} occupata: il listener degli agenti usa la porta {port}");
            }
            IpcStatus { port: Some(port), preferred_port: preferred, error: None }
        }
        Err(e) => {
            log::error!("listener degli agenti non avviato (porta {preferred}): {e}");
            IpcStatus {
                port: None,
                preferred_port: preferred,
                error: Some(format!("impossibile aprire il listener locale: {e}")),
            }
        }
    }
}

/// Posti per le connessioni in corso: `try_acquire` ne prende uno, il guard
/// lo rilascia al drop.
#[derive(Clone)]
struct ConnectionSlots {
    used: Arc<AtomicUsize>,
    max: usize,
}

struct SlotGuard(Arc<AtomicUsize>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ConnectionSlots {
    fn new(max: usize) -> Self {
        Self { used: Arc::new(AtomicUsize::new(0)), max }
    }

    fn try_acquire(&self) -> Option<SlotGuard> {
        self.used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < self.max).then_some(n + 1))
            .ok()
            .map(|_| SlotGuard(self.used.clone()))
    }
}

/// Serve una connessione: legge una riga, inoltra le RPC a `on_rpc` e scrive
/// la risposta; le notifiche vanno a `on_notify`, che decide la riga di
/// risposta.
fn serve_conn<R, N>(stream: &mut TcpStream, on_rpc: R, on_notify: N) -> Result<(), String>
where
    R: FnOnce(RpcRequest) -> RpcResponse,
    N: FnOnce(NotifyPayload) -> Result<NotifyReply, String>,
{
    match read_wire(stream)? {
        Wire::Notify(p) => {
            let result = on_notify(p);
            // Risposta a best effort: i client vecchi hanno gia' chiuso.
            let reply = result.as_ref().cloned().unwrap_or_default();
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let _ = stream.write_all(format!("{}\n", reply.to_line()).as_bytes()).and_then(|_| stream.flush());
            result.map(|_| ())
        }
        Wire::Rpc(req) => {
            let resp = on_rpc(req);
            stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
            stream
                .write_all(format!("{}\n", resp.to_line()).as_bytes())
                .and_then(|_| stream.flush())
                .map_err(|e| e.to_string())
        }
    }
}

/// Elabora una notifica: il cancello decide se ammetterla, l'handler reagisce
/// e, se non e' uno stop in pausa, `forward` la porta al frontend. La risposta
/// porta l'eventuale blocco dello stop.
fn process_notify(
    gate: &Mutex<NotifyGate>,
    handler: &dyn IpcHandler,
    payload: NotifyPayload,
    now: Instant,
    forward: impl FnOnce(&NotifyPayload) -> Result<(), String>,
) -> Result<NotifyReply, String> {
    let admitted = gate.lock().unwrap_or_else(PoisonError::into_inner).admit(payload, handler.session_token(), now);
    let Some(payload) = admitted else {
        return Ok(NotifyReply::default());
    };
    let block = handler.hook_event(&payload);
    if forwards_to_frontend(&payload, block.is_some()) {
        forward(&payload)?;
    }
    Ok(NotifyReply { block })
}

fn handle_conn(
    stream: &mut TcpStream,
    app: &AppHandle,
    handler: &dyn IpcHandler,
    gate: &Mutex<NotifyGate>,
) -> Result<(), String> {
    serve_conn(
        stream,
        |req| handler.rpc(&req),
        |payload| {
            process_notify(gate, handler, payload, Instant::now(), |p| {
                app.emit(FRONTEND_EVENT, p).map_err(|e| e.to_string())
            })
        },
    )
}

/// Ciclo di accettazione: un thread per connessione (un client che non scrive
/// nulla non blocca gli altri), al massimo `MAX_CONNECTIONS` insieme.
fn accept_loop(listener: TcpListener, app: AppHandle, handler: Arc<dyn IpcHandler>) {
    let slots = ConnectionSlots::new(MAX_CONNECTIONS);
    let gate = Arc::new(Mutex::new(NotifyGate::default()));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            // Errore transitorio dell'accept (es. troppi file aperti): non girare a vuoto.
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let Some(slot) = slots.try_acquire() else {
            log::warn!("troppe connessioni al listener degli agenti (max {MAX_CONNECTIONS}): connessione scartata");
            continue;
        };
        let (app, handler, gate) = (app.clone(), handler.clone(), gate.clone());
        let spawned = std::thread::Builder::new().name("ipc-conn".to_string()).spawn(move || {
            let _slot = slot;
            let _ = handle_conn(&mut stream, &app, handler.as_ref(), &gate);
        });
        if let Err(e) = spawned {
            log::warn!("thread di connessione non avviato: {e}");
        }
    }
}

/// Apre il listener (sincrono: parte nel setup, prima di ogni pty, cosi' la
/// porta e' nota quando si costruisce l'ambiente delle sessioni) e avvia il
/// ciclo di accettazione in un thread dedicato.
pub fn start_listener(app: &AppHandle, handler: Arc<dyn IpcHandler>) {
    let bound = bind_listener(DEFAULT_PORT);
    let status = status_of(DEFAULT_PORT, &bound);
    let _ = STATUS.set(status);
    app.manage(IpcState { handler: handler.clone() });
    let Ok(listener) = bound else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || accept_loop(listener, app, handler));
}

/// Self-test del canale: lancia questo stesso exe in modalita' --notify
/// (stesso percorso degli hook, stdin nulla) e verifica l'exit code. Il ding
/// arriva poi via evento al frontend come prova udibile.
#[tauri::command]
pub async fn test_agent_notify(state: tauri::State<'_, IpcState>) -> Result<String, String> {
    let token = state.handler.session_token().to_string();
    crate::blocking(move || self_test_blocking(&token)).await
}

fn self_test_blocking(token: &str) -> Result<String, String> {
    let port =
        listening_port().ok_or_else(|| ipc_status().error.unwrap_or_else(|| "listener non avviato".to_string()))?;
    let exe = std::env::current_exe().map_err(|e| format!("percorso exe: {e}"))?;
    let mut cmd = std::process::Command::new(&exe);
    crate::platform::hide_window(&mut cmd);
    // Il figlio vede il token e la porta dell'app come un terminale di cli-companion,
    // ma non il tab dell'eventuale terminale da cui l'app e' stata avviata.
    let status = cmd
        .args(["--notify", "test", "self-test"])
        .env(TOKEN_ENV, token)
        .env(PORT_ENV, port.to_string())
        .env_remove(SESSION_ENV)
        .stdin(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("self-test non avviabile: {e}"))?;
    if status.success() {
        Ok("segnale inviato: se senti il ding, il canale funziona.".to_string())
    } else {
        Err(format!("self-test fallito (exit {:?}): l'app e' in ascolto?", status.code()))
    }
}
