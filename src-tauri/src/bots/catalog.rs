//! Gli agenti che un Bot puo' usare: quelli registrati, con quali sono
//! abilitati. Passato come parametro a validazione e risoluzione, cosi' restano
//! pure e i test usano integrazioni finte.

use crate::agents::{registry, AgentIntegration, AgentUnavailable};

pub struct AgentCatalog<'a> {
    registered: &'a [&'a dyn AgentIntegration],
    enabled: Vec<String>,
}

impl<'a> AgentCatalog<'a> {
    pub fn new(registered: &'a [&'a dyn AgentIntegration], enabled: Vec<String>) -> Self {
        Self { registered, enabled }
    }

    /// L'integrazione `id`, se e' registrata e abilitata.
    pub fn find(&self, id: &str) -> Result<&'a dyn AgentIntegration, AgentUnavailable> {
        let integration = self
            .registered
            .iter()
            .copied()
            .find(|i| i.id() == id)
            .ok_or_else(|| AgentUnavailable::Unknown(id.to_string()))?;
        if !self.enabled.iter().any(|e| e == id) {
            return Err(AgentUnavailable::Disabled { id: id.to_string(), name: integration.display_name() });
        }
        Ok(integration)
    }
}

impl AgentCatalog<'static> {
    /// Il registro del processo e le integrazioni abilitate dall'utente.
    pub fn current() -> Self {
        Self::new(registry::all(), registry::enabled().iter().map(|i| i.id().to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    #[test]
    fn only_registered_and_enabled_agents_are_found() {
        let (on, off) = (FakeIntegration::full("fake-on"), FakeIntegration::full("fake-off"));
        let registered = [&on as &dyn AgentIntegration, &off];
        let catalog = AgentCatalog::new(&registered, vec!["fake-on".into()]);
        assert_eq!(catalog.find("fake-on").map(|i| i.id()).ok(), Some("fake-on"));
        assert_eq!(
            catalog.find("fake-off").err(),
            Some(AgentUnavailable::Disabled { id: "fake-off".into(), name: "Fake" })
        );
        assert_eq!(catalog.find("gone").err(), Some(AgentUnavailable::Unknown("gone".into())));
    }
}
