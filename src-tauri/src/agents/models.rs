//! Modelli nel vocabolario del core: validita', fasce ed effort ammessi, senza
//! nominare nessun modello. I nomi vengono dal contratto `ModelCatalog`
//! dell'integrazione, i livelli da `EffortLevels`.

use super::contracts::{AgentIntegration, Effort, ModelTier, SuggestedModel};

/// Il modello consigliato per ciascuna fascia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierModels {
    pub standard: &'static str,
    pub strong: &'static str,
}

/// Il nome `model` e' valido per l'agente: lo dice il suo contratto, se lo
/// dichiara; altrimenti vale la forma sicura come argomento.
pub fn model_is_valid(integration: &dyn AgentIntegration, model: &str) -> bool {
    match integration.capabilities().models {
        Some(catalog) => catalog.is_valid(model),
        None => crate::validate::valid_model(model),
    }
}

/// L'agente sa applicare il livello `effort` (lo dichiara `EffortLevels`).
pub fn effort_is_supported(integration: &dyn AgentIntegration, effort: Effort) -> bool {
    integration.capabilities().effort.is_some_and(|levels| levels.levels().contains(&effort))
}

/// L'avviso per un effort che l'agente non sa applicare e che quindi si toglie
/// (in inglese: arriva anche al modello).
pub fn unsupported_effort_warning(integration: &dyn AgentIntegration, effort: Effort) -> String {
    format!("{} does not support the effort level \"{}\": ignored", integration.display_name(), effort.as_str())
}

/// Il primo modello consigliato di ogni fascia; `None` se l'agente non
/// dichiara il contratto o una fascia resta senza consiglio.
pub fn tier_models(integration: &dyn AgentIntegration) -> Option<TierModels> {
    first_per_tier(integration.capabilities().models?.suggested())
}

fn first_per_tier(suggested: &[SuggestedModel]) -> Option<TierModels> {
    let first = |tier| suggested.iter().find(|m| m.tier == tier).map(|m| m.id);
    Some(TierModels { standard: first(ModelTier::Standard)?, strong: first(ModelTier::Strong)? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::{FakeIntegration, FAKE_MODEL_LARGE, FAKE_MODEL_SMALL};

    #[test]
    fn validity_comes_from_the_contract_when_declared() {
        let fake = FakeIntegration::full("fake-full");
        assert!(model_is_valid(&fake, FAKE_MODEL_SMALL));
        // Forma sicura, ma non un nome dell'agente finto.
        assert!(!model_is_valid(&fake, "other-model"));
        assert!(!model_is_valid(&fake, "fake a"));
    }

    #[test]
    fn validity_falls_back_to_the_safe_shape_without_the_contract() {
        let bare = FakeIntegration::bare("fake-bare", "fake --yes");
        assert!(model_is_valid(&bare, "other-model"));
        assert!(!model_is_valid(&bare, "a b"));
        assert!(!model_is_valid(&bare, ""));
    }

    #[test]
    fn tier_models_are_the_first_suggestion_of_each_tier() {
        let fake = FakeIntegration::full("fake-full");
        assert_eq!(tier_models(&fake), Some(TierModels { standard: FAKE_MODEL_SMALL, strong: FAKE_MODEL_LARGE }));
    }

    #[test]
    fn a_tier_without_suggestions_gives_none() {
        let model = |id, tier| SuggestedModel { id, label: id, tier };
        let only_standard = [model("a", ModelTier::Standard), model("b", ModelTier::Standard)];
        assert_eq!(first_per_tier(&only_standard), None);
        let both = [only_standard[0], model("c", ModelTier::Strong), model("d", ModelTier::Strong)];
        assert_eq!(first_per_tier(&both), Some(TierModels { standard: "a", strong: "c" }));
        assert_eq!(first_per_tier(&[]), None);
    }

    #[test]
    fn no_tier_models_without_the_contract() {
        assert_eq!(tier_models(&FakeIntegration::bare("fake-bare", "fake --yes")), None);
    }
}
