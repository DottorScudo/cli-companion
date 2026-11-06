//! Scelta dell'agente per un uso: espliciti -> workspace -> predefinito
//! globale, con il perche' quando non si puo' usare. Puro: lo stato e il
//! registro arrivano come parametri (`registry::resolve_agent` li prende dal
//! processo).

use super::contracts::{AgentIntegration, Capability};
use super::enablement::{default_in, view, IntegrationsState};
use std::fmt;

/// A cosa serve l'agente: decide quale contratto deve dichiarare. Ogni uso
/// ne richiede uno; i terminali interattivi non passano di qui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentUse {
    /// Le sessioni automatiche dei worker.
    Worker,
}

impl AgentUse {
    /// Contratto che l'integrazione deve dichiarare per questo uso.
    pub fn required(self) -> Capability {
        match self {
            AgentUse::Worker => Capability::Automation,
        }
    }

    /// L'uso come compare negli errori (in inglese: li legge anche il modello).
    fn description(self) -> &'static str {
        match self {
            AgentUse::Worker => "automated sessions",
        }
    }
}

/// Perche' un agente non si puo' usare. Il testo e' in inglese: arriva al
/// modello dei worker via RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentUnavailable {
    Unknown(String),
    Disabled { id: String, name: &'static str },
    MissingContract { id: String, name: &'static str, purpose: AgentUse },
}

impl fmt::Display for AgentUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentUnavailable::Unknown(id) => write!(f, "unknown agent: {id}"),
            AgentUnavailable::Disabled { name, .. } => {
                write!(f, "{name} is disabled: enable it in the integrations settings")
            }
            AgentUnavailable::MissingContract { name, purpose, .. } => {
                write!(f, "{name} does not support {}", purpose.description())
            }
        }
    }
}

impl std::error::Error for AgentUnavailable {}

/// L'agente per `purpose`: quello `explicit`, altrimenti quello del
/// workspace, altrimenti il predefinito globale. Vince il primo `Some`: una
/// scelta esplicita sconosciuta o disabilitata e' un errore, mai un ripiego
/// silenzioso su un altro agente.
pub fn resolve_in<'a>(
    registered: &[&'a dyn AgentIntegration],
    state: &IntegrationsState,
    explicit: Option<&str>,
    workspace_agent: Option<&str>,
    purpose: AgentUse,
) -> Result<&'a dyn AgentIntegration, AgentUnavailable> {
    let state = view(state, registered);
    let integration = match explicit.or(workspace_agent) {
        Some(id) => {
            let found = registered.iter().copied().find(|i| i.id() == id);
            let integration = found.ok_or_else(|| AgentUnavailable::Unknown(id.to_string()))?;
            if !state.enabled.iter().any(|e| e == id) {
                return Err(AgentUnavailable::Disabled { id: id.to_string(), name: integration.display_name() });
            }
            integration
        }
        None => default_in(registered, &state),
    };
    if integration.capabilities().declared().contains(&purpose.required()) {
        Ok(integration)
    } else {
        Err(AgentUnavailable::MissingContract {
            id: integration.id().to_string(),
            name: integration.display_name(),
            purpose,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::agents::testing::FakeIntegration;

    const FAKE_ID: &str = "fake-second";

    fn state(enabled: &[&str], default_agent: &str) -> IntegrationsState {
        IntegrationsState {
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
            default_agent: default_agent.into(),
        }
    }

    fn first_id() -> &'static str {
        registry::all()[0].id()
    }

    fn id_of(result: Result<&dyn AgentIntegration, AgentUnavailable>) -> Result<String, AgentUnavailable> {
        result.map(|i| i.id().to_string())
    }

    #[test]
    fn explicit_wins_over_workspace_over_the_global_default() {
        let fake = FakeIntegration::full(FAKE_ID);
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let both = state(&[first_id(), FAKE_ID], first_id());
        let resolve =
            |explicit, workspace| id_of(resolve_in(&registered, &both, explicit, workspace, AgentUse::Worker));
        assert_eq!(resolve(None, None), Ok(first_id().to_string()));
        assert_eq!(resolve(None, Some(FAKE_ID)), Ok(FAKE_ID.to_string()));
        assert_eq!(resolve(Some(first_id()), Some(FAKE_ID)), Ok(first_id().to_string()));
        assert_eq!(resolve(Some(FAKE_ID), Some(first_id())), Ok(FAKE_ID.to_string()));
        // Il predefinito globale e' quello configurato, non il primo del registro.
        let fake_default = state(&[first_id(), FAKE_ID], FAKE_ID);
        assert_eq!(
            id_of(resolve_in(&registered, &fake_default, None, None, AgentUse::Worker)),
            Ok(FAKE_ID.to_string())
        );
    }

    #[test]
    fn a_disabled_choice_is_an_error_never_a_silent_fallback() {
        let fake = FakeIntegration::full(FAKE_ID);
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let only_first = state(&[first_id()], first_id());
        let err = resolve_in(&registered, &only_first, Some(FAKE_ID), Some(first_id()), AgentUse::Worker).err();
        assert_eq!(err, Some(AgentUnavailable::Disabled { id: FAKE_ID.into(), name: "Fake" }));
        assert_eq!(err.unwrap().to_string(), "Fake is disabled: enable it in the integrations settings");
        let from_workspace = resolve_in(&registered, &only_first, None, Some(FAKE_ID), AgentUse::Worker);
        assert!(matches!(from_workspace, Err(AgentUnavailable::Disabled { .. })));
    }

    #[test]
    fn an_unknown_agent_is_reported_with_its_id() {
        let registered = [registry::all()[0]];
        let err = resolve_in(&registered, &IntegrationsState::default(), Some("nope"), None, AgentUse::Worker).err();
        assert_eq!(err, Some(AgentUnavailable::Unknown("nope".into())));
        assert_eq!(err.unwrap().to_string(), "unknown agent: nope");
        let from_workspace =
            resolve_in(&registered, &IntegrationsState::default(), None, Some("gone"), AgentUse::Worker);
        assert_eq!(from_workspace.err(), Some(AgentUnavailable::Unknown("gone".into())));
    }

    #[test]
    fn the_contract_for_the_use_is_required() {
        let bare = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &bare as &dyn AgentIntegration];
        let both = state(&[first_id(), FAKE_ID], first_id());
        let purpose = AgentUse::Worker;
        let err = resolve_in(&registered, &both, Some(FAKE_ID), None, purpose).err().unwrap();
        assert_eq!(err, AgentUnavailable::MissingContract { id: FAKE_ID.into(), name: "Fake", purpose });
        assert_eq!(err.to_string(), "Fake does not support automated sessions");
        // Anche il predefinito globale deve avere il contratto.
        let bare_default = state(&[first_id(), FAKE_ID], FAKE_ID);
        assert!(resolve_in(&registered, &bare_default, None, None, AgentUse::Worker).is_err());
    }

    #[test]
    fn required_contracts_per_use() {
        assert_eq!(AgentUse::Worker.required(), Capability::Automation);
    }
}
