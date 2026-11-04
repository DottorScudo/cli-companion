//! Terminali: una shell per tab dentro un PTY, con output su un canale
//! dedicato. `registry` tiene le sessioni (limite e unicita' del tab sotto
//! lo stesso lock), `spawn` costruisce comando e PTY, `env` l'ambiente,
//! `io` i thread dei flussi, `exit` la chiusura all'uscita dell'app.

mod env;
mod exit;
mod io;
mod registry;
mod spawn;
#[cfg(test)]
mod tests;

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::workspaces::WorkspaceStore;
use io::{spawn_reader, spawn_writer};
use registry::{PtySession, Reservation, SharedRegistry};
use spawn::{check_start_dir, open_pty, pty_size, session_command, start_dir};

/// Messaggi sul canale dedicato di una sessione: `{kind:"output",data}` o
/// `{kind:"exit"}`. Un canale per terminale (invece di un evento globale)
/// evita che ogni tab riceva e scarti l'output di tutti gli altri.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PtyEvent {
    Output { data: String },
    Exit,
}

#[derive(Default)]
pub struct PtyState {
    registry: SharedRegistry,
}

/// Richiesta di apertura di un terminale, come arriva dal frontend.
struct SpawnRequest {
    workspace_id: String,
    tab_id: String,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
}

/// Async: lo spawn verifica la cartella WSL con `wsl.exe` (secondi a VM
/// spenta) e avvia la shell; sul main thread congelerebbe la finestra.
#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    workspace_id: String,
    tab_id: String,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    on_event: Channel<PtyEvent>,
) -> Result<u32, String> {
    if !crate::ipc::is_valid_tab_id(&tab_id) {
        return Err("id tab non valido".to_string());
    }
    let request = SpawnRequest { workspace_id, tab_id, cols, rows, cwd };
    crate::blocking(move || open_session(&app, &request, on_event)).await
}

/// Prenota lo slot, avvia la shell e la rende viva. Se un passo fallisce la
/// prenotazione si libera da sola (drop).
fn open_session(app: &AppHandle, request: &SpawnRequest, on_event: Channel<PtyEvent>) -> Result<u32, String> {
    let registry = &app.state::<PtyState>().registry;
    let ws = app.state::<WorkspaceStore>().find(&request.workspace_id).ok_or("workspace non trovato")?;
    let target = ws.target()?;
    let reservation = Reservation::new(registry, &request.tab_id, &request.workspace_id).map_err(|e| e.to_string())?;

    let dir = start_dir(request.cwd.as_deref(), &ws.path);
    check_start_dir(&target, &dir)?;
    // Il segreto si emette solo a prenotazione riuscita: se un passo seguente
    // fallisce, il drop della credenziale lo revoca.
    let credential = app.state::<crate::automation::AutomationState>().issue_credential(&request.tab_id)?;
    let cmd = session_command(&target, &dir, ws.shell.as_deref(), &request.tab_id, credential.secret());
    let opened = open_pty(cmd, pty_size(request.cols, request.rows))?;

    let session_id = reservation.id();
    reservation.commit(PtySession {
        master: Arc::new(Mutex::new(opened.master)),
        resize_seq: Arc::new(AtomicU64::new(0)),
        writer: spawn_writer(opened.writer),
        child: Mutex::new(opened.child),
    })?;

    let registry = registry.clone();
    // La credenziale vive quanto il flusso della shell: la revoca avviene alla
    // fine dell'output (shell uscita o `pty_close`), dopo `reap`. Finche' la
    // shell e' viva, i suoi figli possono autenticarsi con quel segreto.
    spawn_reader(opened.reader, on_event, move || {
        reap(&registry, session_id);
        drop(credential);
    });
    Ok(session_id)
}

/// Fine del flusso della shell: toglie la sessione dalla tabella (se non l'ha
/// gia' fatto `pty_close`) e ne attende il processo.
fn reap(registry: &SharedRegistry, session_id: u32) {
    let session = registry.lock().unwrap().take_session(session_id);
    if let Some(session) = session {
        let _ = session.child.lock().unwrap().wait();
    }
}

/// Id del workspace in cui e' aperto il tab `tab_id` (None se non ha un pty vivo).
pub fn workspace_of_tab(app: &AppHandle, tab_id: &str) -> Option<String> {
    let state = app.state::<PtyState>();
    let registry = state.registry.lock().unwrap();
    registry.workspace_of_tab(tab_id).map(str::to_string)
}

/// Chiude tutti i terminali all'uscita dell'app, con un'attesa massima breve.
pub fn close_all_on_exit(app: &AppHandle) {
    exit::close_all(&app.state::<PtyState>().registry, exit::EXIT_CLOSE_TIMEOUT);
}

/// Async: gira sul runtime e non sul main thread. Accoda i byte (O(1)) e
/// delega altrove la notifica all'automazione, che prende i suoi lock.
#[tauri::command]
pub async fn pty_write(app: AppHandle, pty: State<'_, PtyState>, session_id: u32, data: String) -> Result<(), String> {
    let (writer, tab_id) =
        pty.registry.lock().unwrap().writer(session_id).ok_or_else(|| "sessione non trovata".to_string())?;
    // Invio nel terminale di un worker che aspettava l'utente: ha risposto.
    if data.contains('\r') {
        tauri::async_runtime::spawn_blocking(move || crate::automation::user_input(&app, &tab_id));
    }
    writer.send(data.into_bytes()).map_err(|_| "sessione chiusa".to_string())
}

#[tauri::command]
pub async fn pty_resize(pty: State<'_, PtyState>, session_id: u32, cols: u16, rows: u16) -> Result<(), String> {
    // Il lock della mappa e' rilasciato prima del resize (puo' essere lento).
    let (master, seq) = pty
        .registry
        .lock()
        .unwrap()
        .session(session_id)
        .map(|sess| (sess.master.clone(), sess.resize_seq.clone()))
        .ok_or_else(|| "sessione non trovata".to_string())?;
    let mine = seq.fetch_add(1, Ordering::SeqCst) + 1;
    crate::blocking(move || {
        let master = master.lock().unwrap();
        // Superato da un resize piu' recente: lo applichera' quello.
        if seq.load(Ordering::SeqCst) != mine {
            return Ok(());
        }
        master.resize(pty_size(cols, rows)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn pty_close(pty: State<'_, PtyState>, session_id: u32) -> Result<(), String> {
    let session = pty.registry.lock().unwrap().take_session(session_id);
    if let Some(session) = session {
        // kill e drop del master (ClosePseudoConsole) possono bloccarsi:
        // fuori dal main thread.
        crate::blocking(move || {
            session.shutdown();
            Ok(())
        })
        .await?;
    }
    Ok(())
}
