//! Punti d'ingresso dall'app reale: RPC dal socket, eventi degli hook, input
//! dell'utente, uscita dall'app, caricamento delle board all'avvio e viste
//! correnti delle board (la base degli eventi dell'Agents HUB all'accensione),
//! creazione dei task di pianificazione, decisione delle proposte e risposta alle
//! domande dei worker per l'HUB.

use super::*;

/// Punto d'ingresso delle RPC dal socket: `rpc::handle` con lo stato reale.
pub fn handle_rpc(app: &AppHandle, req: &RpcRequest) -> RpcResponse {
    rpc::handle(&app.state::<AutomationState>(), &TauriHost(app.clone()), req)
}

/// Evento di un hook di una sessione autenticata (vedi `ipc`). Ritorna il
/// motivo di blocco se lo Stop hook di un worker va fatto proseguire.
pub fn on_hook_event(app: &AppHandle, payload: &NotifyPayload) -> Option<String> {
    let tab = payload.tab_id.as_deref()?;
    let hook = HookEvent {
        event: &payload.event,
        wait: payload.wait_kind(),
        stop_hook_active: payload.stop_hook_active,
        background_work: payload.background_work,
        agent_session_id: payload.agent_session_id.as_deref(),
        source: payload.source.as_deref(),
        reason: payload.reason.as_deref(),
    };
    let st = app.state::<AutomationState>();
    st.run(&TauriHost(app.clone()), |r| r.on_hook(tab, &hook, now_millis()))
}

/// Il core visto dal canale `ipc`: autenticazione con il segreto di sessione, RPC
/// (`handle_rpc`) ed eventi hook (`on_hook_event`).
pub struct IpcBridge(pub AppHandle);

impl crate::ipc::IpcHandler for IpcBridge {
    fn authenticate(&self, session: &str, secret: &str) -> bool {
        self.0.state::<AutomationState>().keys.verify(session, secret)
    }

    fn probe_credential(&self) -> Result<Box<dyn crate::ipc::Credential>, String> {
        let credential = self.0.state::<AutomationState>().keys.issue_probe()?;
        Ok(Box::new(credential))
    }

    fn rpc(&self, req: &RpcRequest) -> RpcResponse {
        handle_rpc(&self.0, req)
    }

    fn hook_event(&self, payload: &NotifyPayload) -> Option<String> {
        on_hook_event(&self.0, payload)
    }
}

/// L'utente ha premuto Invio nel terminale del tab (chiamato da `pty_write`).
pub fn user_input(app: &AppHandle, tab: &str) {
    let st = app.state::<AutomationState>();
    st.run(&TauriHost(app.clone()), |r| r.user_input(tab, now_millis()));
}

/// Attesa massima, in tutto, della fine delle esecuzioni E2E annullate
/// all'uscita: copre `process::KILL_WAIT` (10 s).
const E2E_EXIT_WAIT: Duration = Duration::from_secs(12);

/// Chiusura dell'app: annulla le esecuzioni E2E in corso (uccidendone l'albero
/// di processi), poi salva le board con un salvataggio ancora in coda. Prima
/// le E2E, cosi' l'esito "cancelled" finisce sul task salvato.
pub fn flush_on_exit(app: &AppHandle) {
    let st = app.state::<AutomationState>();
    st.e2e.cancel_all_and_wait(E2E_EXIT_WAIT);
    st.flush_pending_saves(&TauriHost(app.clone()));
}

/// Carica le board salvate (all'avvio dell'app): migra i file di uno schema
/// vecchio e fa fallire come interrotti i task `running`. Un file corrotto o
/// di una versione piu' recente viene messo da parte come backup datato (avviso
/// `store-warning`); uno illeggibile resta dov'e' e non viene riscritto.
pub fn load_boards(app: &tauri::App) {
    let dir = match worktrees::boards_dir(app.handle()) {
        Ok(dir) => dir,
        Err(e) => {
            log::error!("board: cartella dati non disponibile: {e}");
            return;
        }
    };
    let st = app.state::<AutomationState>();
    for board in board_files::load_all(&dir, now_millis()) {
        st.load_board(board);
    }
}

/// Le viste correnti delle board di tutti i workspace, senza pubblicarle.
/// `lib.rs` la registra come sorgente dell'Agents HUB (`hub::set_board_source`),
/// che all'accensione ne fa la base degli eventi della board.
pub fn board_views(app: &AppHandle) -> Vec<BoardView> {
    app.state::<AutomationState>().views(&TauriHost(app.clone()))
}

/// Crea un task `plan` in coda nel workspace `ws_id` (vedi
/// `planner::create_hub_plan_task`): l'API con cui l'Agents HUB delega la
/// pianificazione, con il Bot scelto dal segretario. Ritorna l'id del task. Fa
/// git: da chiamare fuori dal main thread.
pub fn queue_hub_plan_task(app: &AppHandle, ws_id: &str, title: &str, task: &str, bot: &str) -> Result<String, String> {
    planner::create_hub_plan_task(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, title, task, bot)
}

/// Consegna la risposta dell'utente a una domanda di un worker (`ask_user`): la segna
/// sul task e la digita nel terminale del worker (vedi `questions`). L'API con cui
/// l'Agents HUB risponde per conto dell'utente. Non blocca: la scrittura nel
/// terminale si accoda.
pub fn answer_question(
    app: &AppHandle,
    ws_id: &str,
    task_id: &str,
    question_id: &str,
    text: &str,
) -> Result<(), String> {
    app.state::<AutomationState>().answer_question(&TauriHost(app.clone()), ws_id, task_id, question_id, text)
}

/// Accetta o scarta una proposta per conto dell'Agents HUB: lo stesso codice
/// (e la stessa validazione) dei pulsanti del pannello Task. Fa git: da
/// chiamare fuori dal main thread.
pub fn decide_hub_proposal(app: &AppHandle, ws_id: &str, proposal_id: &str, accept: bool) -> Result<(), String> {
    boards::proposal_decide(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, proposal_id, accept)
        .map(drop)
}

/// Un'azione su un task per conto dell'Agents HUB (i segretari): lo stesso
/// codice (e la stessa validazione) del comando `task_action` del pannello Task.
pub fn hub_task_action(
    app: &AppHandle,
    ws_id: &str,
    task_id: &str,
    action: board::TaskAction,
) -> Result<BoardView, String> {
    boards::task_action(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, task_id, action)
}

/// Cambia titolo, specifica, dipendenze o modello di un task `queued` o `failed`
/// per conto dell'Agents HUB: lo stesso codice (e la stessa validazione) del
/// comando `task_update` del pannello Task.
pub fn hub_task_update(app: &AppHandle, ws_id: &str, task_id: &str, patch: &Value) -> Result<BoardView, String> {
    boards::task_update(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, task_id, patch)
}

/// Verifica una modifica senza applicarla: l'errore che `hub_task_update` darebbe adesso.
pub fn hub_task_update_check(app: &AppHandle, ws_id: &str, task_id: &str, patch: &Value) -> Result<(), String> {
    boards::task_update_check(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, task_id, patch)
}

/// Sposta un task `queued` prima di `before_id` per conto dell'Agents HUB, come
/// il comando `task_move`.
pub fn hub_task_move(app: &AppHandle, ws_id: &str, task_id: &str, before_id: &str) -> Result<BoardView, String> {
    boards::task_move(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, task_id, Some(before_id))
}

/// Pausa e `max_parallel` della board per conto dell'Agents HUB, con gli stessi
/// limiti del comando `board_set`.
pub fn hub_board_set(
    app: &AppHandle,
    ws_id: &str,
    max_parallel: Option<u8>,
    paused: Option<bool>,
) -> Result<BoardView, String> {
    boards::board_set(&app.state::<AutomationState>(), &TauriHost(app.clone()), ws_id, max_parallel, paused)
}
