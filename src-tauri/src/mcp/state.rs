//! Stato del server MCP condiviso fra il ciclo di stdin e il thread che
//! riprova `whoami`: ruolo, workspace e quel che serve a decidere se avvisare
//! il client che la lista dei tool e' cambiata.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::{Backend, ROLE_TIMEOUT};
use crate::automation::{lock, Role};

/// Tentativi di `whoami` in background: al massimo una serie per processo.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Retry {
    #[default]
    Idle,
    /// `initialize` e' finito senza ruolo e il backend puo' riprendersi.
    Wanted,
    Started,
}

#[derive(Debug, Default)]
struct State {
    role: Option<Role>,
    /// Workspace della sessione (da `whoami`), a parole: serve alle istruzioni.
    workspace: Option<String>,
    /// Arrivata `notifications/initialized`: da qui il server puo' notificare.
    initialized: bool,
    /// Deciso di rispondere a `tools/list` con una lista vuota, risposta non
    /// ancora scritta: la notifica non deve precederla.
    empty_list_pending: bool,
    /// L'ultima lista che il client ha ricevuto e' vuota.
    client_list_empty: bool,
    retry: Retry,
}

/// Maniglia dello stato: i cloni condividono lo stesso stato.
#[derive(Debug, Default, Clone)]
pub struct Server {
    state: Arc<Mutex<State>>,
}

/// Ruolo e workspace dalla risposta di `whoami`.
fn parse_whoami(v: &Value) -> Result<(Role, Option<String>), String> {
    let role = v
        .get("role")
        .and_then(Value::as_str)
        .and_then(Role::parse)
        .ok_or_else(|| "unknown role in the whoami response".to_string())?;
    let workspace = v.get("workspace").and_then(|w| {
        let field = |k: &str| w.get(k).and_then(Value::as_str);
        Some(format!("{} ({}, {})", field("name")?, field("kind")?, field("path")?))
    });
    Ok((role, workspace))
}

impl Server {
    /// Ruolo della sessione: risolto una volta sola, finche' `whoami` fallisce
    /// si riprova alla chiamata successiva. La RPC parte senza lock, cosi' le
    /// due parti del server non si aspettano; se entrambe riescono vale la prima.
    pub(super) fn role(&self, backend: &dyn Backend) -> Result<Role, String> {
        if let Some(role) = lock(&self.state).role {
            return Ok(role);
        }
        let (role, workspace) = parse_whoami(&backend.rpc("whoami", &json!({}), ROLE_TIMEOUT)?)?;
        let state = &mut *lock(&self.state);
        if state.role.is_none() {
            state.workspace = workspace;
        }
        Ok(*state.role.get_or_insert(role))
    }

    pub(super) fn workspace(&self) -> Option<String> {
        lock(&self.state).workspace.clone()
    }

    pub(super) fn mark_initialized(&self) {
        lock(&self.state).initialized = true;
    }

    /// `initialize` e' finito senza ruolo: se il backend puo' riprendersi
    /// servono i tentativi in background (una serie sola).
    pub(super) fn want_retry(&self) {
        let mut state = lock(&self.state);
        if state.retry == Retry::Idle {
            state.retry = Retry::Wanted;
        }
    }

    /// Prende in carico i tentativi chiesti da `want_retry`: true una volta sola.
    pub(super) fn claim_retry(&self) -> bool {
        let mut state = lock(&self.state);
        let wanted = state.retry == Retry::Wanted;
        if wanted {
            state.retry = Retry::Started;
        }
        wanted
    }

    /// Risposta a `tools/list` decisa: con i tool o vuota.
    pub(super) fn tools_listed(&self, empty: bool) {
        let mut state = lock(&self.state);
        state.empty_list_pending = empty;
        if !empty {
            state.client_list_empty = false;
        }
    }

    /// Una risposta e' stata scritta: una lista vuota ora e' nota al client.
    /// True se va emessa `notifications/tools/list_changed`.
    pub(super) fn reply_written(&self) -> bool {
        let mut state = lock(&self.state);
        if std::mem::take(&mut state.empty_list_pending) {
            state.client_list_empty = true;
        }
        drop(state);
        self.take_list_changed()
    }

    /// True (una volta sola) se il client ha una lista vuota, il ruolo ora e'
    /// noto e il ciclo di vita permette di notificare. Il client, avvisato,
    /// chiede di nuovo la lista: da li' in poi non e' piu' vuota.
    pub(super) fn take_list_changed(&self) -> bool {
        let mut state = lock(&self.state);
        let notify = state.role.is_some() && state.initialized && state.client_list_empty;
        if notify {
            state.client_list_empty = false;
        }
        notify
    }
}
