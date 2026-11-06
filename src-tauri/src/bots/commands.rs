//! Comandi Tauri dei Bot-agente. La validazione (pura) sta fuori dal lock; la
//! modifica passa da `agent_bots` e si salva in `bots.json` prima di essere
//! pubblicata. Ogni comando che cambia l'elenco restituisce le impostazioni salvate.

use super::agent::{AgentBot, AgentBotDraft};
use super::agent_bots;
use super::catalog::AgentCatalog;
use super::skills::{selectable_skills, BotSkillOption};
use super::validate::validate_draft;
use super::{update, BotSettings, BotSettingsStore};
use crate::hub::settings::HubSettingsStore;
use crate::store::data_dir;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn list_agent_bots(state: State<BotSettingsStore>) -> Vec<AgentBot> {
    state.read(|s| s.agents.clone())
}

#[tauri::command]
pub async fn create_agent_bot(app: AppHandle, draft: AgentBotDraft) -> Result<BotSettings, String> {
    crate::blocking(move || {
        let draft = validate_draft(draft, &AgentCatalog::current())?;
        let taken = secretary_names(&app);
        update(&app, |s| agent_bots::create(&mut s.agents, draft, &taken).map(drop))
    })
    .await
}

#[tauri::command]
pub async fn update_agent_bot(app: AppHandle, id: String, draft: AgentBotDraft) -> Result<BotSettings, String> {
    crate::blocking(move || {
        let draft = validate_draft(draft, &AgentCatalog::current())?;
        let taken = secretary_names(&app);
        update(&app, |s| agent_bots::update(&mut s.agents, &id, draft, &taken))
    })
    .await
}

/// Rifiuta il Bot `general`.
#[tauri::command]
pub async fn delete_agent_bot(app: AppHandle, id: String) -> Result<BotSettings, String> {
    crate::blocking(move || update(&app, |s| agent_bots::delete(&mut s.agents, &id))).await
}

/// Riporta compito, modello, effort e skill del Bot al suo predefinito.
#[tauri::command]
pub async fn restore_agent_bot_preset(app: AppHandle, id: String) -> Result<BotSettings, String> {
    crate::blocking(move || update(&app, |s| agent_bots::restore_preset(&mut s.agents, &id))).await
}

/// Ricrea i Bot predefiniti che mancano.
#[tauri::command]
pub async fn restore_agent_bot_presets(app: AppHandle) -> Result<BotSettings, String> {
    crate::blocking(move || {
        let taken = secretary_names(&app);
        update(&app, |s| {
            agent_bots::restore_presets(&mut s.agents, &taken);
            Ok(())
        })
    })
    .await
}

/// Le skill che si possono dare a un Bot: del plugin e dell'utente.
#[tauri::command]
pub async fn list_bot_skills(app: AppHandle) -> Result<Vec<BotSkillOption>, String> {
    crate::blocking(move || {
        let store = crate::user_skills::store_dir(&data_dir(&app)?);
        Ok(selectable_skills(crate::user_skills::list(&store)?))
    })
    .await
}

/// I nomi dei segretari dell'HUB, in minuscolo: un Bot non li puo' prendere.
fn secretary_names(app: &AppHandle) -> Vec<String> {
    app.state::<HubSettingsStore>().read(|s| s.secretaries.iter().map(|sec| sec.name.to_lowercase()).collect())
}
