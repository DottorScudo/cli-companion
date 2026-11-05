//! L'effetto di `request_task_action` sull'app: la causa del turno, poi la
//! vista della board e la scheda scritta nel thread sotto il lock dei thread,
//! solo se l'epoca del motore e' ancora quella del lavoro (vedi `confirm`).

use super::super::board_ops;
use super::super::confirm::{self, RequestParams, NOT_A_WORKSPACE_THREAD};
use super::{AppTurnHost, ThreadIo, STALE, WORKSPACE_NOT_FOUND};
use crate::board::TaskAction;
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, hub_dirs_of};
use crate::hub::engine::runtime::SecretaryEngine;
use crate::hub::events::HubFeed;
use crate::hub::state::HubState;
use serde_json::Value;
use tauri::Manager;

/// Il motivo, per il modello, di una scheda che non si e' potuta scrivere.
const NOT_POSTED: &str = "The request could not be posted.";

impl AppTurnHost<'_> {
    pub(super) async fn request_action(
        &self,
        task_id: String,
        action: TaskAction,
        reason: String,
    ) -> Result<Value, String> {
        board_ops::require_user_request(self.cause)?;
        self.io.post_request(&self.secretary.name, task_id, action, reason).await
    }
}

impl ThreadIo {
    /// Scrive nel thread la scheda della richiesta di `secretary` e ne ritorna
    /// l'esito per il modello; poi `hub-update`.
    async fn post_request(
        &self,
        secretary: &str,
        task_id: String,
        action: TaskAction,
        reason: String,
    ) -> Result<Value, String> {
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
            let params = RequestParams { task_id: &task_id, action, reason: &reason, secretary: &secretary };
            let (value, touched) = state.update(&dirs, &key.channel, |list| {
                ensure_channel(&app, &key.channel).map_err(|e| {
                    log::warn!("hub: canale della richiesta di {secretary} non trovato: {e}");
                    NOT_POSTED.to_string()
                })?;
                confirm::post_request(list, &key.thread_id, view, params, &mut |prefix| state.next_id(prefix, now), now)
            })?;
            emit_update(&app, &touched);
            Ok(value)
        })
        .await
    }
}
