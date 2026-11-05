//! Gli effetti di `task_action` e `board_control` sull'app: la causa del
//! turno e il perimetro (`tools::change_workspace`), poi fuori dal main thread e solo se l'epoca del
//! motore e' ancora quella del lavoro, la vista corrente della board e la
//! scrittura con le azioni registrate da `lib.rs` (vedi `board_ops`).

use super::super::board_ops::{self, BoardChange, BoardWriter, TaskOp};
use super::super::tools::change_workspace;
use super::{AppTurnHost, ThreadIo, STALE, WORKSPACE_NOT_FOUND};
use crate::board::{BoardView, TaskAction};
use crate::hub::actions::{BoardActions, HubBoardActions};
use crate::hub::engine::runtime::SecretaryEngine;
use crate::hub::events::HubFeed;
use serde_json::Value;
use tauri::{AppHandle, Manager};

impl AppTurnHost<'_> {
    pub(super) async fn change_task(
        &self,
        workspace: Option<String>,
        task_id: String,
        op: TaskOp,
    ) -> Result<Value, String> {
        let (id, name) = self.changeable_board(workspace)?;
        self.io.write_board(id, move |board, view| board_ops::change_task(board, view, &name, &task_id, &op)).await
    }

    pub(super) async fn control_board(&self, workspace: Option<String>, change: BoardChange) -> Result<Value, String> {
        let (id, name) = self.changeable_board(workspace)?;
        self.io.write_board(id, move |board, view| board_ops::control_board(board, view, &name, change)).await
    }

    /// Id e nome del workspace la cui board il turno puo' cambiare: solo in un
    /// turno svegliato dall'utente.
    fn changeable_board(&self, workspace: Option<String>) -> Result<(String, String), String> {
        board_ops::require_user_request(self.cause)?;
        let requested = self.requested_workspace(workspace)?;
        let id = change_workspace(requested, self.channel(), self.secretary).map_err(|e| self.access_error(e))?;
        let name = self.names.name_of(&id).unwrap_or(&id).to_string();
        Ok((id, name))
    }
}

impl ThreadIo {
    /// Esegue `write` sulla vista corrente della board `workspace`, fuori dal
    /// main thread e solo se l'HUB non e' stato spento nel frattempo.
    async fn write_board<F>(&self, workspace: String, write: F) -> Result<Value, String>
    where
        F: FnOnce(&AppBoard<'_>, &BoardView) -> Result<Value, String> + Send + 'static,
    {
        let (app, epoch) = (self.app.clone(), self.epoch);
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let board = AppBoard { app: &app, actions: app.state::<HubBoardActions>().get()? };
            let views = app.state::<HubFeed>().current_views(&app);
            let view =
                views.iter().find(|v| v.workspace_id == workspace).ok_or_else(|| WORKSPACE_NOT_FOUND.to_string())?;
            write(&board, view)
        })
        .await
    }
}

/// Le board dell'app, scritte con le azioni registrate da `lib.rs`.
struct AppBoard<'a> {
    app: &'a AppHandle,
    actions: BoardActions,
}

impl BoardWriter for AppBoard<'_> {
    fn act_on_task(&self, workspace: &str, task_id: &str, action: TaskAction) -> Result<BoardView, String> {
        (self.actions.act_on_task)(self.app, workspace, task_id, action)
    }

    fn move_task(&self, workspace: &str, task_id: &str, before: &str) -> Result<BoardView, String> {
        (self.actions.move_task)(self.app, workspace, task_id, before)
    }

    fn set_board(&self, workspace: &str, change: BoardChange) -> Result<BoardView, String> {
        (self.actions.set_board)(self.app, workspace, change.max_parallel, change.paused)
    }
}
