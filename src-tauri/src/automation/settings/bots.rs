//! I Bot-agente in `get_workspace_settings.bots`: ogni Bot (anche disabilitato)
//! con cio' che il modello deve sapere per sceglierlo e proporlo nei task,
//! compresi i tipi di task che puo' eseguire (`kinds`).

use crate::agents::{registry, AgentIntegration, Readiness};
use crate::bots::resolve::resolve_bot;
use crate::bots::{AgentBot, AgentCatalog};
use serde_json::{json, Value};

/// Gli agenti abilitati con la loro prontezza nel workspace, gia' controllata
/// (una sola volta per agente: il controllo lancia processi).
pub(super) type ProbedAgents<'a> = [(&'a dyn AgentIntegration, Readiness)];

/// I Bot nell'ordine dell'elenco. L'agente e' quello che il Bot usa nel
/// workspace (`workspace_agent` o il predefinito globale), con il suo errore se
/// non e' utilizzabile; `readiness` viene dai controlli gia' fatti per `agents`.
pub(super) fn bots_json(bots: &[AgentBot], workspace_agent: Option<&str>, probed: &ProbedAgents) -> Vec<Value> {
    let workspace_agent = workspace_agent.unwrap_or_else(|| registry::default_agent().id());
    let catalog = AgentCatalog::current();
    bots.iter().map(|bot| bot_json(bot, workspace_agent, &catalog, probed)).collect()
}

fn bot_json(bot: &AgentBot, workspace_agent: &str, catalog: &AgentCatalog, probed: &ProbedAgents) -> Value {
    let resolved = resolve_bot(bot, workspace_agent, catalog);
    let (agent, model) = match &resolved {
        Ok(resolved) => (Some(resolved.agent_id.as_str()), resolved.model.as_deref()),
        Err(_) => (None, None),
    };
    let readiness = agent.and_then(|id| probed.iter().find(|(probed, _)| probed.id() == id)).map(|(_, state)| state);
    json!({
        "id": bot.id,
        "name": bot.name,
        "task": bot.task,
        "enabled": bot.enabled,
        "agent": agent,
        "agent_error": resolved.as_ref().err().map(ToString::to_string),
        "readiness": readiness,
        "model": model,
        "model_choice": bot.model,
        "effort": bot.effort,
        "skill": bot.skill,
        "kinds": bot.kinds,
    })
}
