//! Gli effetti reali del motore su thread e board, fuori dal main thread
//! (`crate::blocking`) e senza tenere lock durante le chiamate al modello.
//! `ThreadIo` legge e scrive il thread di un lavoro; `AppTurnHost` implementa
//! `TurnHost` per i tool di un segretario. Ogni scrittura controlla prima che
//! l'epoca del motore sia ancora quella del lavoro (HUB non spento nel frattempo).
//!
//! `open_plan_task` crea il task `plan` tenendo il lock dei thread: cosi' il
//! thread degli eventi della board, che aspetta lo stesso lock, trova il task
//! gia' collegato al thread del segretario e non gli apre un thread suo. Il
//! lock dura la creazione del task (qualche comando git), mai una chiamata al
//! modello; l'automazione non prende mai il lock dei thread.

use super::board_ops::{BoardChange, TaskOp};
use super::board_query::BoardQuery;
use super::chat::ModelError;
use super::openai::OpenAiModel;
use super::output;
use super::planning::{self, PlanTicket, ProposalBoard, ProposalRef};
use super::relay::{self, QuestionSelector, RelayCandidates};
use super::runtime::SecretaryEngine;
use super::sessions::{ThreadKey, WakeCause};
use super::task_page::TaskPage;
use super::thread_page::ThreadPage;
use super::tools::{board_workspace, BoardAccessError};
use super::turn::TurnHost;
use super::workspace_names::WorkspaceNames;
use crate::board::BoardView;
use crate::hub::actions::{AppAnswers, BoardActions, HubBoardActions};
use crate::hub::api_key::ApiKey;
use crate::hub::append::{append_text, TextMessage};
use crate::hub::attachments::commands::agent_paths;
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, hub_dirs_of};
use crate::hub::events::HubFeed;
use crate::hub::model::{Author, RequestedAction, Thread, ThreadState};
use crate::hub::questions::QuestionRef;
use crate::hub::settings::{Provider, Secretary};
use crate::hub::state::HubState;
use crate::hub::text::clean_line;
use crate::hub::threads::{summary, THREAD_NOT_FOUND};
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Il lavoro e' di un'epoca passata: l'HUB e' stato spento.
const STALE: &str = "sessione chiusa: l'Agents HUB è stato spento";
/// Il workspace chiesto non c'e' piu' fra le board, per il modello.
const WORKSPACE_NOT_FOUND: &str = "That workspace was not found.";
/// Caratteri tenuti dell'id di un task nei messaggi di errore.
const TASK_ID_KEPT: usize = 64;

/// Il thread di un lavoro del motore.
pub struct ThreadIo {
    app: AppHandle,
    key: ThreadKey,
    epoch: u64,
}

impl ThreadIo {
    pub fn new(app: &AppHandle, key: &ThreadKey, epoch: u64) -> Self {
        Self { app: app.clone(), key: key.clone(), epoch }
    }

    pub async fn load_thread(&self) -> Result<Thread, String> {
        let (app, key) = (self.app.clone(), self.key.clone());
        crate::blocking(move || {
            let dirs = hub_dirs_of(&app)?;
            app.state::<HubState>()
                .read(&dirs, &key.channel, |list| list.thread(&key.thread_id).cloned())
                .ok_or_else(|| THREAD_NOT_FOUND.to_string())
        })
        .await
    }

    /// Scrive un messaggio di testo nel thread (con `hub-update`).
    pub async fn append(&self, author: Author, body: String) -> Result<(), String> {
        let (app, key, epoch) = (self.app.clone(), self.key.clone(), self.epoch);
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            append_text(&app, &key.channel, &key.thread_id, TextMessage { author, body, reply_to: None }).map(|_| ())
        })
        .await
    }

    /// Un messaggio di sistema; un fallimento resta nel log.
    pub async fn notice(&self, text: String) {
        if let Err(e) = self.append(Author::System, text).await {
            log::error!("hub {}: avviso del motore non scritto: {e}", self.key.thread_id);
        }
    }

    /// Sostituisce il riassunto del thread (con `hub-update` se cambia).
    pub async fn save_summary(&self, text: String) -> Result<(), String> {
        let (app, key, epoch) = (self.app.clone(), self.key.clone(), self.epoch);
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let dirs = hub_dirs_of(&app)?;
            let touched = app.state::<HubState>().update_when(
                &dirs,
                &key.channel,
                |list| {
                    ensure_channel(&app, &key.channel)?;
                    let changed = list.set_summary(&key.thread_id, text)?;
                    Ok(if changed { list.thread(&key.thread_id).map(summary) } else { None })
                },
                Option::is_some,
            )?;
            if let Some(touched) = touched {
                emit_update(&app, &touched);
            }
            Ok(())
        })
        .await
    }

    /// `open_plan_task`: crea il task `plan` e lo collega al thread, con
    /// `secretary` titolare e il Bot `bot` (vedi `planning::open_plan`), poi `hub-update`.
    pub async fn open_plan(&self, trigger: &str, secretary: &str, goal: String, bot: String) -> Result<Value, String> {
        let (app, key, epoch) = (self.app.clone(), self.key.clone(), self.epoch);
        let (trigger, secretary) = (trigger.to_string(), secretary.to_string());
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let actions = app.state::<HubBoardActions>().get()?;
            let dirs = hub_dirs_of(&app)?;
            let now = chrono::Utc::now().timestamp_millis();
            let bots = crate::bots::agent_bots(&app);
            let ticket = PlanTicket {
                channel: &key.channel,
                thread_id: &key.thread_id,
                trigger_id: &trigger,
                secretary: &secretary,
                goal: &goal,
                bot: &bot,
                bots: &bots,
                attachments: agent_paths(&app, &dirs.attachments, &key.channel),
            };
            let (value, touched) = app.state::<HubState>().update(&dirs, &key.channel, |list| {
                ensure_channel(&app, &key.channel)?;
                let value =
                    planning::open_plan(list, &ticket, now, |ws_id, order| (actions.create_plan)(&app, ws_id, order))?;
                Ok((value, list.thread(&key.thread_id).map(summary)))
            })?;
            if let Some(touched) = touched {
                emit_update(&app, &touched);
            }
            Ok(value)
        })
        .await
    }

    /// `decide_proposal` sulla proposta `target` del turno (vedi `planning::decide`).
    pub async fn decide_proposal(&self, target: Option<ProposalRef>, accept: bool) -> Result<Value, String> {
        let (app, epoch) = (self.app.clone(), self.epoch);
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let board = AppProposals { actions: app.state::<HubBoardActions>().get()?, app: app.clone() };
            planning::decide(&board, target.as_ref(), accept)
        })
        .await
    }

    /// `relay_answer` sulla domanda `target` del turno (vedi `relay::relay`);
    /// l'avviso di cosa e' stato inviato lo scrive il turno (`audit::notice`).
    pub async fn relay_answer(&self, target: Result<QuestionRef, &'static str>, body: String) -> Result<Value, String> {
        let (app, epoch) = (self.app.clone(), self.epoch);
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            relay::relay(&AppAnswers { app: &app }, target.as_ref().map_err(|e| *e), &body)
        })
        .await
    }

    /// Il client del modello con il provider e la API key correnti.
    pub async fn model(&self, provider: &Provider) -> Result<OpenAiModel, ModelError> {
        if !provider.is_configured() {
            return Err(ModelError::NotConfigured);
        }
        let app = self.app.clone();
        let key = crate::blocking(move || app.state::<ApiKey>().read()).await.map_err(|e| {
            log::error!("hub: API key del provider non leggibile: {e}");
            ModelError::KeyUnavailable
        })?;
        OpenAiModel::new(provider, key)
    }
}

/// Le proposte delle board viste da `decide_proposal`: le viste correnti (dalla
/// sorgente registrata) e la decisione registrata da `lib.rs`.
struct AppProposals {
    app: AppHandle,
    actions: BoardActions,
}

impl ProposalBoard for AppProposals {
    fn pending_tasks(&self, workspace: &str, proposal_id: &str) -> Option<usize> {
        let views = self.app.state::<HubFeed>().current_views(&self.app);
        let view = views.iter().find(|v| v.workspace_id == workspace)?;
        view.proposals.iter().find(|p| p.id == proposal_id).map(|p| p.tasks.len())
    }

    fn decide(&self, workspace: &str, proposal_id: &str, accept: bool) -> Result<(), String> {
        (self.actions.decide_proposal)(&self.app, workspace, proposal_id, accept)
    }
}

/// Gli effetti dei tool di un segretario sul suo thread.
pub struct AppTurnHost<'a> {
    pub io: &'a ThreadIo,
    pub secretary: &'a Secretary,
    /// Il messaggio che ha svegliato il turno.
    pub trigger: &'a str,
    /// Perche' il turno e' stato svegliato: solo un messaggio dell'utente puo'
    /// cambiare la coda (`board_ops::require_user_request`).
    pub cause: WakeCause,
    /// La proposta che il turno puo' decidere (`planning::proposal_target`).
    pub proposal: Option<ProposalRef>,
    /// La domanda a cui il turno puo' rispondere, o perche' non ce n'e' una
    /// (`relay::relay_target`).
    pub question: RelayCandidates,
    /// I workspace all'inizio del turno, per parlarne col nome.
    pub names: &'a WorkspaceNames,
}

impl AppTurnHost<'_> {
    fn channel(&self) -> &Channel {
        &self.io.key.channel
    }

    /// Le viste correnti delle board (dall'automazione, fuori da ogni lock dell'HUB).
    async fn views(&self) -> Result<Vec<BoardView>, String> {
        let app = self.io.app.clone();
        crate::blocking(move || Ok(app.state::<HubFeed>().current_views(&app))).await.map_err(|e| {
            log::warn!("hub: viste delle board non lette: {e}");
            "The boards could not be read.".to_string()
        })
    }
}

impl AppTurnHost<'_> {
    /// La board del workspace chiesto (`None` = quello del canale del thread),
    /// se il segretario puo' leggerla; l'errore e' in inglese, per il modello.
    async fn readable_board(&self, workspace: Option<String>) -> Result<BoardView, String> {
        let requested = self.requested_workspace(workspace)?;
        let id = board_workspace(requested, self.channel(), self.secretary).map_err(|e| self.access_error(e))?;
        let views = self.views().await?;
        views.into_iter().find(|v| v.workspace_id == id).ok_or_else(|| WORKSPACE_NOT_FOUND.to_string())
    }

    /// L'id del workspace a cui il modello si riferisce col nome (`None` = nessuno).
    fn requested_workspace(&self, workspace: Option<String>) -> Result<Option<String>, String> {
        workspace.map(|reference| self.names.find_for_model(&reference, self.secretary)).transpose()
    }

    /// Perche' la board non si puo' leggere o cambiare, per il modello.
    fn access_error(&self, error: BoardAccessError) -> String {
        let name = |id: &str| self.names.name_of(id).unwrap_or("unknown").to_string();
        match error {
            BoardAccessError::NoChannelBoard => format!(
                "This thread is in the General channel, which has no board: pass `workspace` with the name of one of: {}.",
                self.names.readable_names(self.secretary).join(", ")
            ),
            BoardAccessError::NotAllowed(id) => {
                format!("You cannot use the board of workspace \"{}\": it is not one of your channels.", name(&id))
            }
            BoardAccessError::OutsideThread(id) => format!(
                "You can change only the board of this thread's channel, not the one of workspace \"{}\". Tell the \
                user to ask in that workspace's channel or in the General channel.",
                name(&id)
            ),
        }
    }
}

impl TurnHost for AppTurnHost<'_> {
    async fn board_status(&self, workspace: Option<String>, query: BoardQuery) -> Result<Value, String> {
        let view = self.readable_board(workspace).await?;
        Ok(output::board_status(&view, self.names, &query))
    }

    async fn board_overview(&self) -> Result<Value, String> {
        let views = self.views().await?;
        Ok(output::board_overview(&views, self.names, self.secretary))
    }

    async fn task_details(
        &self,
        workspace: Option<String>,
        task_id: String,
        page: Option<TaskPage>,
    ) -> Result<Value, String> {
        let view = self.readable_board(workspace).await?;
        let task = view.tasks.iter().find(|v| v.task.id == task_id).ok_or_else(|| {
            format!(
                "No task with id \"{}\" on the board of workspace \"{}\". Use `board_status` to find task ids.",
                clean_line(&task_id, TASK_ID_KEPT),
                self.names.name_of(&view.workspace_id).unwrap_or("unknown")
            )
        })?;
        output::task_details(task, &view.workspace_id, self.names, page.as_ref())
    }

    async fn thread_read(&self, page: ThreadPage) -> Result<Value, String> {
        let thread = self.io.load_thread().await.map_err(|e| {
            log::warn!("hub: thread_read fallito: {e}");
            "The thread could not be read.".to_string()
        })?;
        output::thread(&thread, self.names, &page)
    }

    async fn post_message(&self, body: String) -> Result<(), String> {
        self.io.append(Author::Bot { name: self.secretary.name.clone() }, body).await.map_err(|e| {
            log::warn!("hub: post_message di {} fallito: {e}", self.secretary.name);
            "The message could not be posted.".to_string()
        })
    }

    async fn open_plan_task(&self, goal: String, bot: String) -> Result<Value, String> {
        self.io.open_plan(self.trigger, &self.secretary.name, goal, bot).await
    }

    async fn decide_proposal(&self, accept: bool) -> Result<Value, String> {
        self.io.decide_proposal(self.proposal.clone(), accept).await
    }

    async fn relay_answer(&self, selector: QuestionSelector, body: String) -> Result<Value, String> {
        self.io.relay_answer(relay::choose(&self.question, &selector), body).await
    }

    async fn task_action(&self, workspace: Option<String>, task_id: String, op: TaskOp) -> Result<Value, String> {
        self.change_task(workspace, task_id, op).await
    }

    async fn board_control(&self, workspace: Option<String>, change: BoardChange) -> Result<Value, String> {
        self.control_board(workspace, change).await
    }

    async fn request_task_action(
        &self,
        task_id: String,
        action: RequestedAction,
        reason: String,
    ) -> Result<Value, String> {
        self.request_action(task_id, action, reason).await
    }

    async fn request_task_update(&self, task_id: String, patch: Value, reason: String) -> Result<Value, String> {
        self.request_update(task_id, patch, reason).await
    }

    async fn set_thread_state(&self, state: ThreadState) -> Result<Value, String> {
        self.change_thread_state(state).await
    }

    async fn notice(&self, text: String) {
        self.io.notice(text).await;
    }
}

mod board_writes;
mod requests;
mod thread_state;
