//! Task Manager: coda di task per workspace, scheduler deterministico e worker
//! (tab PTY marcati automatici) che consegnano da soli. I worker parlano con
//! l'app solo tramite tool MCP (`get_task`, `complete_task`) e gli hook del
//! coding agent; i tab normali (Intake compreso) propongono task e consultano
//! la coda. Contratto: `docs/BOARD.MD`.
//!
//! Struttura a strati, pensata per i test:
//! - `Registry` (`registry`): nucleo PURO (niente `AppHandle`, niente I/O, tempo
//!   iniettato). Tiene worker e board; gli effetti da eseguire li accumula come
//!   `Effect`. La macchina a stati dei worker (hook, chiusura del tab) sta in
//!   `worker`.
//! - `AutomationState` (`state`): `Mutex<Registry>` (managed state Tauri). `run`
//!   esegue una modifica, schedula le board toccate e applica gli effetti
//!   FUORI dal lock (avvio dei task con git, salvataggi, eventi).
//! - `Host` (`host`): gli effetti verso l'esterno (eventi Tauri, timer, git,
//!   workspace). `TauriHost` e' l'implementazione reale; i test usano un host
//!   finto, cosi' anche `dispatch` si prova senza finestra.
//!
//! Ogni terminale ha un segreto suo (`auth::SessionKeys`, emesso all'apertura
//! del pty e revocato quando la shell finisce): autentica le chiamate RPC e gli
//! eventi degli hook insieme all'id del tab, cosi' un processo non puo'
//! spacciarsi per un'altra sessione.
//!
//! File di questo modulo:
//! - `auth`: segreti per sessione e autenticazione delle richieste RPC.
//! - `worker`: tipi dei worker e macchina a stati (hook, chiusura del tab).
//! - `registry`: `Registry`, effetti e operazioni di base sui worker.
//! - `bridge`: punti d'ingresso dall'app reale (RPC, hook, uscita, avvio).
//! - `commands`: comandi Tauri invocati dal frontend.
//! - `roles`: tabella unica dei metodi RPC e dei ruoli che possono chiamarli.
//! - `state`, `host`, `rpc`: stato Tauri, effetti verso l'esterno, smistamento RPC.
//! - `boards`, `worktrees`, `review`, `settings`, `skills`, `e2e`, `auto`:
//!   operazioni sulle board e funzioni dei singoli ambiti.

use crate::board::{
    self, build_report, Board, BoardView, FailCause, Outcome, ReportFields, Task, TaskKind, TaskStatus, WorkerActivity,
    WorkerEnd, WorkerSignal, WorktreeInfo, SUMMARY_MAX,
};
use crate::ipc::{is_valid_tab_id, NotifyPayload, RpcRequest, RpcResponse};
pub(crate) use crate::validate::{clip_chars, req_text};
use crate::workspaces::{Delivery, Workspace};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

mod agent_session;
mod auth;
pub mod auto;
mod board_files;
mod boards;
mod bridge;
mod commands;
mod e2e;
mod hooks;
mod host;
mod process;
mod registry;
mod review;
mod roles;
mod rpc;
pub(crate) mod settings;
pub mod skills;
mod state;
mod worker;
mod worktrees;

pub use crate::board::WorkerStatus;
pub use crate::user_skills::targets::SkillTarget;
use auth::authenticate;
pub use auth::SessionCredential;
use auth::SessionKeys;
pub use bridge::{flush_on_exit, handle_rpc, load_boards, on_hook_event, user_input, IpcBridge};
pub use commands::*;
pub use host::{Host, TauriHost};
#[cfg(test)]
use registry::MAX_TRACKED_WORKERS;
pub(crate) use registry::TASK_UNASSIGNED;
pub use registry::{Completion, Effect, Notice, Registry};
pub use roles::{Method, Role};
pub use state::AutomationState;
use worker::Worker;
pub(crate) use worker::WorkerEnv;
pub use worker::{AutomationTabInfo, HookEvent, NewWorker, SpawnEvent, TabEvent, BOOTSTRAP_PROMPT, STOP_BLOCK_REASON};
pub use worktrees::{CleanupTarget, GitOut, StartJob};

/// Dopo `complete_task` il tab resta aperto il tempo di vedere l'esito.
pub const CLOSE_DELAY: Duration = Duration::from_millis(1500);
/// Dopo la chiusura del tab, il tempo perche' Windows rilasci la cartella
/// del worktree prima di rimuoverla.
pub const CLEANUP_DELAY: Duration = Duration::from_secs(2);

/// Prende il lock anche se un altro thread e' andato in panico tenendolo: lo
/// stato resta consistente (il registro cambia solo con chiamate ai suoi metodi)
/// e un panico isolato non deve fermare l'app.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Campo stringa obbligatorio di un oggetto JSON.
pub(crate) fn req_str<'a>(p: &'a Value, key: &str) -> Result<&'a str, String> {
    match p.get(key) {
        Some(Value::String(s)) => Ok(s.as_str()),
        None | Some(Value::Null) => Err(format!("{key} is missing")),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

#[cfg(test)]
mod tests;
