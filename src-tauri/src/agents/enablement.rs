//! Quali integrazioni sono abilitate e quale e' l'agente predefinito: stato e
//! regole, tutto puro (la persistenza sta in `integrations_store.rs`).
//!
//! Invarianti di ogni stato letto con `view`: almeno un'integrazione
//! abilitata, solo id registrati, nell'ordine del registro, senza doppioni, e
//! un agente predefinito che sia fra gli abilitati. Lo stato vuoto (`Default`)
//! significa "non ancora configurato".

use super::contracts::AgentIntegration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stato globale delle integrazioni, salvato in `integrations.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IntegrationsState {
    /// Id delle integrazioni abilitate.
    #[serde(default)]
    pub enabled: Vec<String>,
    /// Id dell'agente predefinito (uno degli abilitati).
    #[serde(default)]
    pub default_agent: String,
}

fn is_registered(registered: &[&dyn AgentIntegration], id: &str) -> bool {
    registered.iter().any(|i| i.id() == id)
}

/// Gli id registrati presenti in `ids`, nell'ordine del registro e senza doppioni.
fn known_in_registry_order(registered: &[&dyn AgentIntegration], ids: &[String]) -> Vec<String> {
    registered.iter().map(|i| i.id()).filter(|id| ids.iter().any(|e| e == id)).map(str::to_string).collect()
}

/// Il predefinito scelto se e' abilitato, altrimenti il primo abilitato.
fn default_among(enabled: &[String], wanted: &str) -> String {
    if enabled.iter().any(|e| e == wanted) {
        wanted.to_string()
    } else {
        enabled.first().cloned().unwrap_or_default()
    }
}

/// Lo stato grezzo riportato alle invarianti, in silenzio: serve a ogni
/// lettura. Se non resta nessuna integrazione abilitata vale la prima del registro.
pub fn view(raw: &IntegrationsState, registered: &[&dyn AgentIntegration]) -> IntegrationsState {
    let mut enabled = known_in_registry_order(registered, &raw.enabled);
    if enabled.is_empty() {
        enabled.extend(registered.first().map(|i| i.id().to_string()));
    }
    let default_agent = default_among(&enabled, &raw.default_agent);
    IntegrationsState { enabled, default_agent }
}

/// Lo stato appena caricato dal disco, con gli avvisi da registrare nel log.
/// Gli id sconosciuti (integrazioni rimosse dall'app) cadono; se non ne resta
/// nessuno (file assente, corrotto, solo id sconosciuti) si abilitano quelle
/// installate (`installed`) o, in mancanza, la prima del registro.
pub fn normalize_loaded(
    raw: &IntegrationsState,
    registered: &[&dyn AgentIntegration],
    installed: impl Fn(&str) -> bool,
) -> (IntegrationsState, Vec<String>) {
    let mut unknown: Vec<&String> = Vec::new();
    for id in raw.enabled.iter().filter(|id| !is_registered(registered, id)) {
        if !unknown.contains(&id) {
            unknown.push(id);
        }
    }
    let mut warnings: Vec<String> =
        unknown.iter().map(|id| format!("integrazione sconosciuta ignorata: {id} (rimossa dall'app)")).collect();

    let mut enabled = known_in_registry_order(registered, &raw.enabled);
    if enabled.is_empty() {
        enabled = registered.iter().map(|i| i.id()).filter(|id| installed(id)).map(str::to_string).collect();
    }
    if enabled.is_empty() {
        enabled.extend(registered.first().map(|i| i.id().to_string()));
    }
    let default_agent = default_among(&enabled, &raw.default_agent);
    if !raw.default_agent.is_empty() && raw.default_agent != default_agent {
        warnings.push(format!("agente predefinito {} non abilitato: usato {default_agent}", raw.default_agent));
    }
    (IntegrationsState { enabled, default_agent }, warnings)
}

fn name_of(registered: &[&dyn AgentIntegration], id: &str) -> Result<&'static str, String> {
    registered
        .iter()
        .find(|i| i.id() == id)
        .map(|i| i.display_name())
        .ok_or_else(|| format!("agente sconosciuto: {id}"))
}

/// Abilita o disabilita `id`. Si rifiuta di spegnere l'ultima integrazione
/// abilitata e quella predefinita (prima se ne sceglie un'altra).
pub fn set_enabled(
    state: &IntegrationsState,
    registered: &[&dyn AgentIntegration],
    id: &str,
    enabled: bool,
) -> Result<IntegrationsState, String> {
    let name = name_of(registered, id)?;
    let mut next = view(state, registered);
    let is_on = next.enabled.iter().any(|e| e == id);
    if enabled && !is_on {
        next.enabled.push(id.to_string());
    } else if !enabled && is_on {
        if next.enabled.len() == 1 {
            return Err("almeno un'integrazione deve restare abilitata".to_string());
        }
        if next.default_agent == id {
            return Err(format!("{name} e' l'agente predefinito: scegline prima un altro"));
        }
        next.enabled.retain(|e| e != id);
    }
    Ok(view(&next, registered))
}

/// Rende `id` l'agente predefinito; deve essere abilitato.
pub fn set_default(
    state: &IntegrationsState,
    registered: &[&dyn AgentIntegration],
    id: &str,
) -> Result<IntegrationsState, String> {
    let name = name_of(registered, id)?;
    let mut next = view(state, registered);
    if !next.enabled.iter().any(|e| e == id) {
        return Err(format!("abilita {name} prima di renderlo predefinito"));
    }
    next.default_agent = id.to_string();
    Ok(next)
}

/// Le integrazioni abilitate, nell'ordine del registro. `registered` non e'
/// vuoto (il registro non lo e' mai).
pub fn enabled_in<'a>(
    registered: &[&'a dyn AgentIntegration],
    state: &IntegrationsState,
) -> Vec<&'a dyn AgentIntegration> {
    let enabled = view(state, registered).enabled;
    registered.iter().copied().filter(|i| enabled.iter().any(|e| e == i.id())).collect()
}

/// L'agente predefinito. `registered` non e' vuoto (il registro non lo e'
/// mai): con un elenco vuoto va in panico.
pub fn default_in<'a>(registered: &[&'a dyn AgentIntegration], state: &IntegrationsState) -> &'a dyn AgentIntegration {
    let default_agent = view(state, registered).default_agent;
    registered.iter().copied().find(|i| i.id() == default_agent).unwrap_or(registered[0])
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

    fn only_fake() -> IntegrationsState {
        state(&[FAKE_ID], FAKE_ID)
    }

    #[test]
    fn view_keeps_registered_ids_in_registry_order_without_duplicates() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let raw = state(&[FAKE_ID, "gone", first_id(), FAKE_ID], FAKE_ID);
        assert_eq!(view(&raw, &registered), state(&[first_id(), FAKE_ID], FAKE_ID));
    }

    #[test]
    fn view_of_an_empty_state_enables_the_first_registered_integration() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let expected = state(&[first_id()], first_id());
        assert_eq!(view(&IntegrationsState::default(), &registered), expected);
        assert_eq!(view(&state(&["gone"], "gone"), &registered), expected);
    }

    #[test]
    fn view_fixes_a_default_that_is_not_enabled() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        assert_eq!(view(&state(&[FAKE_ID], first_id()), &registered), only_fake());
        assert_eq!(view(&state(&[first_id(), FAKE_ID], ""), &registered).default_agent, first_id());
    }

    #[test]
    fn loading_without_a_file_enables_the_installed_integrations() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let empty = IntegrationsState::default();
        let none = normalize_loaded(&empty, &registered, |_| false);
        assert_eq!(none, (state(&[first_id()], first_id()), Vec::new()));
        let one = normalize_loaded(&empty, &registered, |id| id == FAKE_ID);
        assert_eq!(one, (only_fake(), Vec::new()));
        let both = normalize_loaded(&empty, &registered, |_| true);
        assert_eq!(both.0, state(&[first_id(), FAKE_ID], first_id()));
    }

    #[test]
    fn a_newly_registered_integration_is_not_enabled_for_existing_users() {
        // File gia' presente: anche se il nuovo pacchetto risulta installato, non si abilita da solo.
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let existing = state(&[first_id()], first_id());
        let (loaded, warnings) = normalize_loaded(&existing, &registered, |_| true);
        assert_eq!((loaded, warnings), (existing.clone(), Vec::new()));
        assert_eq!(view(&existing, &registered), existing);
        assert_eq!(enabled_in(&registered, &existing).len(), 1);
    }

    #[test]
    fn loading_drops_unknown_ids_with_a_warning_each() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let raw = state(&["gone", FAKE_ID, "also-gone", "gone"], FAKE_ID);
        let (loaded, warnings) = normalize_loaded(&raw, &registered, |_| true);
        assert_eq!(loaded, only_fake());
        assert_eq!(
            warnings,
            vec![
                "integrazione sconosciuta ignorata: gone (rimossa dall'app)".to_string(),
                "integrazione sconosciuta ignorata: also-gone (rimossa dall'app)".to_string()
            ]
        );
        // Solo id sconosciuti: si riparte dalle installate, e il predefinito perso e' segnalato.
        let (loaded, warnings) = normalize_loaded(&state(&["gone"], "gone"), &registered, |id| id == FAKE_ID);
        assert_eq!(loaded, only_fake());
        assert_eq!(warnings.len(), 2);
        assert!(warnings[1].contains("agente predefinito gone non abilitato"));
    }

    #[test]
    fn loading_replaces_a_default_that_is_not_enabled() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let (loaded, warnings) = normalize_loaded(&state(&[FAKE_ID], first_id()), &registered, |_| false);
        assert_eq!(loaded, only_fake());
        assert_eq!(warnings.len(), 1);
        // Uno stato gia' valido non cambia e non produce avvisi.
        let (kept, warnings) = normalize_loaded(&loaded, &registered, |_| false);
        assert_eq!((kept, warnings), (loaded, Vec::new()));
    }

    #[test]
    fn rules_enable_in_registry_order_and_refuse_to_leave_nothing_enabled() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let both = set_enabled(&IntegrationsState::default(), &registered, FAKE_ID, true).unwrap();
        assert_eq!(both, state(&[first_id(), FAKE_ID], first_id()));
        assert_eq!(set_enabled(&both, &registered, FAKE_ID, true).unwrap(), both);
        let switched = set_default(&both, &registered, FAKE_ID).unwrap();
        assert_eq!(set_enabled(&switched, &registered, first_id(), false).unwrap(), only_fake());
        let err = set_enabled(&only_fake(), &registered, FAKE_ID, false).unwrap_err();
        assert_eq!(err, "almeno un'integrazione deve restare abilitata");
        // Spegnere cio' che e' gia' spento non fa nulla.
        assert_eq!(set_enabled(&only_fake(), &registered, first_id(), false).unwrap(), only_fake());
    }

    #[test]
    fn the_default_cannot_be_disabled_until_another_one_is_chosen() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let both = state(&[first_id(), FAKE_ID], first_id());
        let err = set_enabled(&both, &registered, first_id(), false).unwrap_err();
        let name = registry::all()[0].display_name();
        assert_eq!(err, format!("{name} e' l'agente predefinito: scegline prima un altro"));
        let switched = set_default(&both, &registered, FAKE_ID).unwrap();
        assert_eq!(set_enabled(&switched, &registered, first_id(), false).unwrap(), only_fake());
    }

    #[test]
    fn the_default_must_be_enabled_and_known() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let only_first = state(&[first_id()], first_id());
        assert_eq!(
            set_default(&only_first, &registered, FAKE_ID),
            Err("abilita Fake prima di renderlo predefinito".to_string())
        );
        assert_eq!(set_default(&only_first, &registered, "nope"), Err("agente sconosciuto: nope".to_string()));
        assert_eq!(set_enabled(&only_first, &registered, "nope", true), Err("agente sconosciuto: nope".to_string()));
        assert_eq!(set_default(&only_first, &registered, first_id()).unwrap(), only_first);
    }

    #[test]
    fn enabled_and_default_follow_the_state() {
        let fake = FakeIntegration::bare(FAKE_ID, "fake-cli");
        let registered = [registry::all()[0], &fake as &dyn AgentIntegration];
        let ids = |state: &IntegrationsState| enabled_in(&registered, state).iter().map(|i| i.id()).collect::<Vec<_>>();
        assert_eq!(ids(&IntegrationsState::default()), vec![first_id()]);
        assert_eq!(ids(&state(&[FAKE_ID, first_id()], FAKE_ID)), vec![first_id(), FAKE_ID]);
        assert_eq!(default_in(&registered, &state(&[first_id(), FAKE_ID], FAKE_ID)).id(), FAKE_ID);
        assert_eq!(default_in(&registered, &IntegrationsState::default()).id(), first_id());
        assert_eq!(default_in(&registered, &state(&[first_id()], FAKE_ID)).id(), first_id());
    }
}
