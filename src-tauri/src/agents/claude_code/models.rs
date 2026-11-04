//! Contratto `ModelCatalog` di Claude Code: gli alias dei modelli che consiglia.

use super::ClaudeCode;
use crate::agents::{ModelCatalog, ModelTier, SuggestedModel};

const SONNET: &str = "sonnet";
const OPUS: &str = "opus";
const HAIKU: &str = "haiku";

/// In ordine di preferenza dentro ogni fascia.
const SUGGESTED: &[SuggestedModel] = &[
    SuggestedModel { id: SONNET, label: "Sonnet", tier: ModelTier::Standard },
    SuggestedModel { id: OPUS, label: "Opus", tier: ModelTier::Strong },
    SuggestedModel { id: HAIKU, label: "Haiku", tier: ModelTier::Standard },
];

impl ModelCatalog for ClaudeCode {
    fn suggested(&self) -> &'static [SuggestedModel] {
        SUGGESTED
    }

    /// Senza modello esplicito sceglie Claude Code: l'app non passa `--model`.
    fn default_model(&self) -> Option<&'static str> {
        None
    }

    /// Claude Code accetta i suoi alias (`sonnet`, `opus`) e gli id completi
    /// (`claude-opus-4.1`): la regola e' la forma sicura come argomento, non
    /// un elenco chiuso che invecchierebbe a ogni nuovo modello.
    fn is_valid(&self, model: &str) -> bool {
        crate::validate::valid_model(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_sonnet_then_opus_then_haiku() {
        let ids: Vec<_> = ClaudeCode.suggested().iter().map(|m| m.id).collect();
        assert_eq!(ids, [SONNET, OPUS, HAIKU]);
        assert_eq!(ClaudeCode.suggested()[1].tier, ModelTier::Strong);
    }

    #[test]
    fn leaves_the_default_to_claude_code() {
        assert_eq!(ClaudeCode.default_model(), None);
    }

    #[test]
    fn accepts_aliases_and_full_ids_but_not_unsafe_names() {
        for ok in [SONNET, "claude-opus-4.1", "claude-sonnet-4-5_x"] {
            assert!(ClaudeCode.is_valid(ok), "{ok}");
        }
        for bad in ["", "a b", "a;b", "a$b"] {
            assert!(!ClaudeCode.is_valid(bad), "{bad:?}");
        }
    }
}
