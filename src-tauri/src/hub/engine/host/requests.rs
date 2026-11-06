//! L'effetto di `request_task_action` e `request_task_update` sull'app: la
//! causa del turno, poi la vista della board e la scheda scritta nel thread
//! sotto il lock dei thread (per una modifica, dopo averla provata sulla
//! board), solo se l'epoca del motore e' ancora quella del lavoro (vedi
//! `confirm`).

use super::super::board_ops;
use super::super::bot_choice;
use super::super::confirm::{self, RequestParams, NOT_A_WORKSPACE_THREAD};
use super::{AppTurnHost, ThreadIo, STALE, WORKSPACE_NOT_FOUND};
use crate::hub::actions::HubBoardActions;
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, hub_dirs_of};
use crate::hub::engine::runtime::SecretaryEngine;
use crate::hub::events::HubFeed;
use crate::hub::model::RequestedAction;
use crate::hub::state::HubState;
use serde_json::Value;
use tauri::Manager;

/// Il motivo, per il modello, di una scheda che non si e' potuta scrivere.
const NOT_POSTED: &str = "The request could not be posted.";

/// Cosa chiede una richiesta del segretario, per il thread.
struct Ask {
    task_id: String,
    action: RequestedAction,
    /// I campi da cambiare, solo per `update`.
    patch: Option<Value>,
    reason: String,
}

impl AppTurnHost<'_> {
    pub(super) async fn request_action(
        &self,
        task_id: String,
        action: RequestedAction,
        reason: String,
    ) -> Result<Value, String> {
        board_ops::require_user_request(self.cause)?;
        self.io.post_request(&self.secretary.name, Ask { task_id, action, patch: None, reason }).await
    }

    pub(super) async fn request_update(&self, task_id: String, patch: Value, reason: String) -> Result<Value, String> {
        board_ops::require_user_request(self.cause)?;
        let ask = Ask { task_id, action: RequestedAction::Update, patch: Some(patch), reason };
        self.io.post_request(&self.secretary.name, ask).await
    }
}

impl ThreadIo {
    /// Scrive nel thread la scheda della richiesta di `secretary` e ne ritorna
    /// l'esito per il modello; poi `hub-update`.
    async fn post_request(&self, secretary: &str, ask: Ask) -> Result<Value, String> {
        let (app, key, epoch) = (self.app.clone(), self.key.clone(), self.epoch);
        let secretary = secretary.to_string();
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let Channel::Workspace(workspace) = &key.channel else {
                return Err(NOT_A_WORKSPACE_THREAD.to_string());
            };
            // Le viste dall'automazione, fuori dal lock dei thread.
            let views = app.state::<HubFeed>().current_views(&app);
            let view =
                views.iter().find(|v| &v.workspace_id == workspace).ok_or_else(|| WORKSPACE_NOT_FOUND.to_string())?;
            let dirs = hub_dirs_of(&app).map_err(|e| {
                log::warn!("hub: cartelle dell'HUB non disponibili per la richiesta di {secretary}: {e}");
                NOT_POSTED.to_string()
            })?;
            let state = app.state::<HubState>();
            let now = chrono::Utc::now().timestamp_millis();
            let params = RequestParams {
                task_id: &ask.task_id,
                action: ask.action,
                patch: ask.patch.as_ref(),
                reason: &ask.reason,
                secretary: &secretary,
            };
            let (value, touched) = state.update(&dirs, &key.channel, |list| {
                ensure_channel(&app, &key.channel).map_err(|e| {
                    log::warn!("hub: canale della richiesta di {secretary} non trovato: {e}");
                    NOT_POSTED.to_string()
                })?;
                let verify = |patch: &Value| check_update(&app, workspace, &ask.task_id, patch);
                let mut next_id = |prefix| state.next_id(prefix, now);
                confirm::post_request(list, &key.thread_id, view, params, &mut next_id, now, &verify)
            })?;
            emit_update(&app, &touched);
            Ok(value)
        })
        .await
    }
}

/// La modifica e' accettabile adesso (Bot, stato del task, grafo, modello): un
/// grafo non valido si rifiuta gia' alla richiesta, non al clic.
fn check_update(app: &tauri::AppHandle, workspace: &str, task_id: &str, patch: &Value) -> Result<(), String> {
    if let Some(bot) = patch.get("bot").and_then(Value::as_str) {
        bot_choice::check(&crate::bots::agent_bots(app), bot)?;
    }
    let actions = app.state::<HubBoardActions>().get()?;
    (actions.check_task_update)(app, workspace, task_id, patch).map_err(board_ops::refused)
}
