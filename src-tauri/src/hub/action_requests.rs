//! Il comando con cui l'utente decide una richiesta di azione di un segretario
//! (il clic su «Conferma» o «Annulla» della scheda nel thread). La decisione e
//! l'esecuzione sono in `engine::confirm`; qui solo l'I/O: sotto il lock dei
//! thread si legge il task sulla board, si esegue l'azione e si aggiorna la
//! scheda in un colpo solo, cosi' due clic non eseguono due volte. L'automazione
//! non prende mai il lock dei thread (la board avvisa l'HUB con un invio su un
//! canale), quindi eseguire l'azione col lock preso non puo' bloccarsi.
//!
//! Dopo il lock: `hub-update` e il risveglio di chi ha fatto la richiesta,
//! dalla riga dell'esito (`ActionRequestResolved`), anche se non e' il titolare
//! del thread.

use super::actions::{BoardActions, HubBoardActions};
use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, ensure_enabled, hub_dirs_of};
use super::engine::confirm::{resolve, ActionBoard};
use super::engine::SecretaryEngine;
use super::events::{EventWake, HubFeed};
use super::model::HubActionRequest;
use super::state::HubState;
use crate::board::{TaskAction, TaskView};
use tauri::{AppHandle, Manager};

/// Decide la richiesta della scheda `message_id`: `confirm` la esegue, altrimenti
/// la annulla. Una richiesta scaduta o non piu' valida non e' un errore: torna
/// aggiornata (stato e nota) per mostrarla. Una gia' decisa e' un errore.
#[tauri::command]
pub async fn hub_resolve_action_request(
    app: AppHandle,
    channel: String,
    thread_id: String,
    message_id: String,
    confirm: bool,
) -> Result<HubActionRequest, String> {
    let channel = Channel::parse(&channel)?;
    ensure_enabled(&app)?;
    crate::blocking(move || resolve_in_app(&app, &channel, &thread_id, &message_id, confirm)).await
}

fn resolve_in_app(
    app: &AppHandle,
    channel: &Channel,
    thread_id: &str,
    message_id: &str,
    confirm: bool,
) -> Result<HubActionRequest, String> {
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let board = AppActionBoard { app, actions: app.state::<HubBoardActions>().get()? };
    let now = chrono::Utc::now().timestamp_millis();
    let resolved = state.update(&dirs, channel, |list| {
        // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
        ensure_channel(app, channel)?;
        resolve(list, thread_id, message_id, confirm, now, &mut |prefix| state.next_id(prefix, now), &board)
    })?;
    emit_update(app, &resolved.touched);
    let wake = EventWake {
        thread_id: thread_id.to_string(),
        secretary: resolved.request.secretary.clone(),
        trigger: resolved.outcome_id,
    };
    app.state::<SecretaryEngine>().on_events(app, channel, &[wake]);
    Ok(resolved.request)
}

/// La board dell'app: le viste correnti e le azioni registrate da `lib.rs`.
struct AppActionBoard<'a> {
    app: &'a AppHandle,
    actions: BoardActions,
}

impl ActionBoard for AppActionBoard<'_> {
    fn task(&self, workspace: &str, task_id: &str) -> Option<TaskView> {
        let views = self.app.state::<HubFeed>().current_views(self.app);
        let view = views.into_iter().find(|v| v.workspace_id == workspace)?;
        view.tasks.into_iter().find(|t| t.task.id == task_id)
    }

    fn act(&self, workspace: &str, task_id: &str, action: TaskAction) -> Result<(), String> {
        (self.actions.act_on_task)(self.app, workspace, task_id, action).map(drop)
    }
}
