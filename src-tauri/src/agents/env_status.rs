//! Stato di un agente in un target (rilevamento + pacchetto installato) e il
//! sondaggio parallelo di tutte le coppie agente x target.

use super::contracts::AgentIntegration;
use crate::platform::{Target, TargetKind};
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AgentEnvStatus {
    pub agent_id: String,
    pub agent_name: String,
    pub env_key: String,
    pub env_label: String,
    pub env_kind: TargetKind,
    pub found: bool,
    pub version: Option<String>,
    pub installed: bool,
}

/// Rileva l'agente nel target. Lancia processi: fuori dal main thread.
pub(super) fn status_for(integration: &dyn AgentIntegration, target: &Target) -> AgentEnvStatus {
    let det = integration.detect(target);
    let installed =
        det.found && integration.capabilities().package.is_some_and(|package| package.is_installed(target, &det));
    AgentEnvStatus {
        agent_id: integration.id().to_string(),
        agent_name: integration.display_name().to_string(),
        env_key: target.key(),
        env_label: target.label(),
        env_kind: target.kind(),
        found: det.found,
        version: det.version,
        installed,
    }
}

/// Esegue `probe` per ogni coppia agente x target, tutte in parallelo (ogni
/// coppia costa piu' processi e su WSL anche l'avvio della VM). L'ordine e'
/// per target e, dentro, per agente; una sonda andata in panico manca.
pub(super) fn probe_pairs<T: Send>(
    integrations: &[&dyn AgentIntegration],
    targets: &[Target],
    probe: impl Fn(&dyn AgentIntegration, &Target) -> T + Sync,
) -> Vec<T> {
    std::thread::scope(|s| {
        let probe = &probe;
        let handles: Vec<_> = targets
            .iter()
            .flat_map(|target| integrations.iter().map(move |i| (*i, target)))
            .map(|(i, target)| s.spawn(move || probe(i, target)))
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

pub(super) fn collect_status(integrations: &[&dyn AgentIntegration], targets: &[Target]) -> Vec<AgentEnvStatus> {
    probe_pairs(integrations, targets, status_for)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    #[test]
    fn status_covers_every_integration_and_target() {
        let a = FakeIntegration::bare("a", "a-cli");
        let b = FakeIntegration::full("b");
        let targets = [Target::Windows, Target::Wsl { distro: "Ubuntu".into() }];
        let mut status = collect_status(&[&a, &b], &targets);
        status.sort_by(|x, y| (&x.agent_id, &x.env_key).cmp(&(&y.agent_id, &y.env_key)));
        let keys: Vec<_> = status.iter().map(|s| (s.agent_id.as_str(), s.env_key.as_str())).collect();
        assert_eq!(keys, vec![("a", "Ubuntu"), ("a", "windows"), ("b", "Ubuntu"), ("b", "windows")]);
        assert!(status.iter().all(|s| !s.found && !s.installed));
    }

    #[test]
    fn status_reports_what_the_integration_detects() {
        let present = FakeIntegration::versioned("present", "1.0").detected(true, true);
        let agent_only = FakeIntegration::versioned("agent-only", "1.0").detected(true, false);
        let status = collect_status(&[&present, &agent_only], &[Target::Windows]);
        let flags: Vec<_> = status.iter().map(|s| (s.found, s.installed)).collect();
        assert_eq!(flags, vec![(true, true), (true, false)]);
    }

    #[test]
    fn probes_keep_target_then_integration_order() {
        let a = FakeIntegration::bare("a", "a-cli");
        let b = FakeIntegration::bare("b", "b-cli");
        let targets = [Target::Windows, Target::Wsl { distro: "Ubuntu".into() }];
        let order = probe_pairs(&[&a, &b], &targets, |i, t| format!("{}@{}", i.id(), t.key()));
        assert_eq!(order, vec!["a@windows", "b@windows", "a@Ubuntu", "b@Ubuntu"]);
    }
}
