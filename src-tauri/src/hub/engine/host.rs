//! Gli effetti reali del motore su thread e board, fuori dal main thread
//! (`crate::blocking`) e senza tenere lock durante le chiamate al modello.
//! `ThreadIo` legge e scrive il thread di un lavoro; `AppTurnHost` implementa
//! `TurnHost` per i tool di un segretario. Ogni scrittura controlla prima che
//! l'epoca del motore sia ancora quella del lavoro (HUB non spento nel frattempo).

use super::chat::ModelError;
use super::openai::OpenAiModel;
use super::output;
use super::runtime::SecretaryEngine;
use super::sessions::ThreadKey;
use super::tools::{board_workspace, BoardAccessError};
use super::turn::TurnHost;
use crate::board::BoardView;
use crate::hub::api_key::ApiKey;
use crate::hub::append::{append_text, TextMessage};
use crate::hub::channel::Channel;
use crate::hub::commands::{emit_update, ensure_channel, threads_dir_of};
use crate::hub::events::HubFeed;
use crate::hub::model::{Author, Thread};
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

/// Gli effetti dei tool di un segretario sul suo thread.
pub struct AppTurnHost<'a> {
    pub io: &'a ThreadIo,
    pub secretary: &'a Secretary,
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
    async fn board_status(&self, workspace: Option<String>) -> Result<Value, String> {
        match board_workspace(workspace, self.channel(), self.secretary) {
            Ok(id) => {
                let views = self.views().await?;
                let view = views.iter().find(|v| v.workspace_id == id);
                view.map(output::board_status).ok_or_else(|| format!("Workspace `{id}` not found."))
            }
            Err(BoardAccessError::NoChannelBoard) => {
                let readable: Vec<String> = self
                    .views()
                    .await?
                    .into_iter()
                    .map(|v| v.workspace_id)
                    .filter(|id| self.secretary.channels.allows(&Channel::Workspace(id.clone())))
                    .collect();
                Err(format!(
                    "This thread is in the General channel, which has no board: pass `workspace` with one of {readable:?}."
                ))
            }
            Err(BoardAccessError::NotAllowed(id)) => {
                Err(format!("You cannot read the board of workspace `{id}`: it is not one of your channels."))
            }
        }
    }

    async fn thread_read(&self) -> Result<Value, String> {
        self.io.load_thread().await.map(|t| output::thread(&t)).map_err(|e| {
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

    async fn notice(&self, text: String) {
        self.io.notice(text).await;
    }
}
