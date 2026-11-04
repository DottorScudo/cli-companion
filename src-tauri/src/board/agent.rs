//! L'agente di un task: quale agente esegue la specifica e se il suo modello
//! gli appartiene. Gli errori sono in inglese: li leggono i modelli (MCP) e
//! l'utente li vede nell'interfaccia.

use super::TaskSpec;
use crate::agents::{model_is_valid, registry, AgentIntegration, AgentUnavailable, AgentUse};

/// Come si sceglie l'agente di un uso: `explicit`, agente del workspace,
/// predefinito globale (la firma di `registry::resolve_agent`).
type Resolver<'a> =
    dyn Fn(Option<&str>, Option<&str>, AgentUse) -> Result<&'a dyn AgentIntegration, AgentUnavailable> + 'a;

/// L'id dell'agente che eseguira' `spec`: il suo, altrimenti quello del
/// workspace, altrimenti il predefinito globale. L'agente deve poter eseguire
/// i worker e il modello della specifica, se c'e', deve essere uno dei suoi.
pub fn resolve_spec_agent(spec: &TaskSpec, workspace_agent: Option<&str>) -> Result<String, String> {
    resolve_spec_agent_with(&registry::resolve_agent, spec, workspace_agent)
}

fn resolve_spec_agent_with(
    resolve: &Resolver<'_>,
    spec: &TaskSpec,
    workspace_agent: Option<&str>,
) -> Result<String, String> {
    let integration = resolve(spec.agent.as_deref(), workspace_agent, AgentUse::Worker)
        .map_err(|unavailable| format!("task '{}': {unavailable}", spec.id))?;
    if let Some(model) = spec.model.as_deref().filter(|m| !model_is_valid(integration, m)) {
        return Err(format!(
            "task '{}': invalid model for {}: {model} (see get_workspace_settings.agents)",
            spec.id,
            integration.display_name()
        ));
    }
    Ok(integration.id().to_string())
}

#[cfg(test)]
mod tests;
