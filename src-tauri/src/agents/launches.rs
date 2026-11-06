//! Avvii integrati (disponibili in ogni workspace), derivati dal registro: un
//! avvio interattivo per ogni integrazione abilitata. Le sessioni guidate da una
//! skill sono i Bot (`bots::launches`).

use super::contracts::AgentIntegration;
use super::enablement::{enabled_in, IntegrationsState};
use super::integrations_store;
use super::registry;
use serde::Serialize;
use ts_rs::TS;

/// Avvio integrato derivato dal registro.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct BuiltinLaunch {
    /// `builtin:<agent_id>`.
    pub id: String,
    pub name: String,
    pub command: String,
    /// Comando salvato nella sessione del tab per il ripristino.
    pub restore_command: String,
    pub agent_id: String,
    pub agent_name: String,
}

fn agent_launch(integration: &dyn AgentIntegration) -> BuiltinLaunch {
    BuiltinLaunch {
        id: format!("builtin:{}", integration.id()),
        name: integration.display_name().to_string(),
        command: integration.interactive_command().to_string(),
        restore_command: integration.interactive_command().to_string(),
        agent_id: integration.id().to_string(),
        agent_name: integration.display_name().to_string(),
    }
}

/// Come `builtin_launch_list`, su un registro e uno stato dati.
fn launches_in(registered: &[&dyn AgentIntegration], state: &IntegrationsState) -> Vec<BuiltinLaunch> {
    enabled_in(registered, state).into_iter().map(agent_launch).collect()
}

/// Avvii integrati: uno per ogni agente abilitato, nell'ordine del registro.
/// Usata anche per validare gli id.
pub fn builtin_launch_list() -> Vec<BuiltinLaunch> {
    launches_in(registry::all(), &integrations_store::current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    fn ids(launches: &[BuiltinLaunch]) -> Vec<&str> {
        launches.iter().map(|b| b.id.as_str()).collect()
    }

    fn state(enabled: &[&str], default_agent: &str) -> IntegrationsState {
        IntegrationsState {
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
            default_agent: default_agent.into(),
        }
    }

    #[test]
    fn builtin_launches_from_registry() {
        let l = builtin_launch_list();
        assert_eq!(ids(&l), ["builtin:claude-code"]);
        assert_eq!(l[0].name, "Claude Code");
        assert_eq!(l[0].command, l[0].restore_command);
        assert_eq!((l[0].agent_id.as_str(), l[0].agent_name.as_str()), ("claude-code", "Claude Code"));
    }

    #[test]
    fn only_enabled_agents_have_a_launch_and_no_skill_entry_exists() {
        let a = FakeIntegration::full("a");
        let b = FakeIntegration::plugin_skills_only("b", "b-cli");
        let c = FakeIntegration::bare("c", "c-cli");
        let registered: [&dyn AgentIntegration; 3] = [&a, &b, &c];
        assert_eq!(
            ids(&launches_in(&registered, &state(&["a", "b", "c"], "a"))),
            ["builtin:a", "builtin:b", "builtin:c"]
        );
        let l = launches_in(&registered, &state(&["c"], "c"));
        assert_eq!(ids(&l), ["builtin:c"]);
        assert_eq!((l[0].command.as_str(), l[0].restore_command.as_str()), ("c-cli", "c-cli"));
    }
}
