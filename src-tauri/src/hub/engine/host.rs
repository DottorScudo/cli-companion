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

use super::board_query::BoardQuery;
use super::chat::ModelError;
use super::openai::OpenAiModel;
use super::output;
use super::planning::{self, PlanTicket, ProposalBoard, ProposalRef};
use super::relay;
use super::runtime::SecretaryEngine;
use super::sessions::ThreadKey;
use super::texts;
use super::tools::{board_workspace, BoardAccessError};
use super::turn::TurnHost;
use super::workspace_names::WorkspaceNames;
use crate::board::BoardView;
use crate::hub::actions::{AppAnswers, BoardActions, HubBoardActions};
use crate::hub::api_key::ApiKey;
use crate::hub::append::{append_text, TextMessage};
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, threads_dir_of};
use crate::hub::events::HubFeed;
use crate::hub::model::{Author, Thread};
use crate::hub::questions::QuestionRef;
use crate::hub::settings::{Provider, Secretary};
use crate::hub::state::HubState;
use crate::hub::threads::{summary, THREAD_NOT_FOUND};
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Il lavoro e' di un'epoca passata: l'HUB e' stato spento.
const STALE: &str = "sessione chiusa: l'Agents HUB è stato spento";

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
            let dir = threads_dir_of(&app)?;
            app.state::<HubState>()
                .read(&dir, &key.channel, |list| list.thread(&key.thread_id).cloned())
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
            let dir = threads_dir_of(&app)?;
            let touched = app.state::<HubState>().update_when(
                &dir,
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
    /// `secretary` titolare (vedi `planning::open_plan`), poi `hub-update`.
    pub async fn open_plan(&self, trigger: &str, secretary: &str, goal: String) -> Result<Value, String> {
        let (app, key, epoch) = (self.app.clone(), self.key.clone(), self.epoch);
        let (trigger, secretary) = (trigger.to_string(), secretary.to_string());
        crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            let actions = app.state::<HubBoardActions>().get()?;
            let dir = threads_dir_of(&app)?;
            let now = chrono::Utc::now().timestamp_millis();
            let ticket = PlanTicket {
                channel: &key.channel,
                thread_id: &key.thread_id,
                trigger_id: &trigger,
                secretary: &secretary,
                goal: &goal,
            };
            let (value, touched) = app.state::<HubState>().update(&dir, &key.channel, |list| {
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
    /// consegnata, il thread mostra all'utente cosa e' stato inviato.
    pub async fn relay_answer(
        &self,
        target: Result<QuestionRef, &'static str>,
        secretary: &str,
        body: String,
    ) -> Result<Value, String> {
        let (app, epoch) = (self.app.clone(), self.epoch);
        let text = body.clone();
        let value = crate::blocking(move || {
            if !app.state::<SecretaryEngine>().is_current(epoch) {
                return Err(STALE.to_string());
            }
            relay::relay(&AppAnswers { app: &app }, target.as_ref().map_err(|e| *e), &text)
        })
        .await?;
        self.notice(texts::answer_relayed(secretary, &body)).await;
        Ok(value)
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
    /// La proposta che il turno puo' decidere (`planning::proposal_target`).
    pub proposal: Option<ProposalRef>,
    /// La domanda a cui il turno puo' rispondere, o perche' non ce n'e' una
    /// (`relay::relay_target`).
    pub question: Result<QuestionRef, &'static str>,
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

impl TurnHost for AppTurnHost<'_> {
    async fn board_status(&self, workspace: Option<String>, query: BoardQuery) -> Result<Value, String> {
        let requested = workspace.map(|reference| self.names.find_for_model(&reference, self.secretary)).transpose()?;
        match board_workspace(requested, self.channel(), self.secretary) {
            Ok(id) => {
                let views = self.views().await?;
                let view = views.iter().find(|v| v.workspace_id == id);
                view.map(|v| output::board_status(v, self.names, &query)).ok_or_else(|| "That workspace was not found.".to_string())
            }
            Err(BoardAccessError::NoChannelBoard) => Err(format!(
                "This thread is in the General channel, which has no board: pass `workspace` with the name of one of: {}.",
                self.names.readable_names(self.secretary).join(", ")
            )),
            Err(BoardAccessError::NotAllowed(id)) => Err(format!(
                "You cannot read the board of workspace \"{}\": it is not one of your channels.",
                self.names.name_of(&id).unwrap_or("unknown")
            )),
        }
    }

    async fn thread_read(&self) -> Result<Value, String> {
        self.io.load_thread().await.map(|t| output::thread(&t, self.names)).map_err(|e| {
            log::warn!("hub: thread_read fallito: {e}");
            "The thread could not be read.".to_string()
        })
    }

    async fn post_message(&self, body: String) -> Result<(), String> {
        self.io.append(Author::Bot { name: self.secretary.name.clone() }, body).await.map_err(|e| {
            log::warn!("hub: post_message di {} fallito: {e}", self.secretary.name);
            "The message could not be posted.".to_string()
        })
    }

    async fn open_plan_task(&self, goal: String) -> Result<Value, String> {
        self.io.open_plan(self.trigger, &self.secretary.name, goal).await
    }

    async fn decide_proposal(&self, accept: bool) -> Result<Value, String> {
        self.io.decide_proposal(self.proposal.clone(), accept).await
    }

    async fn relay_answer(&self, body: String) -> Result<Value, String> {
        self.io.relay_answer(self.question.clone(), &self.secretary.name, body).await
    }

    async fn notice(&self, text: String) {
        self.io.notice(text).await;
    }
}
