//! Contratto `ModelCatalog` di OpenCode: i modelli sono `provider/modello`
//! (`opencode/gpt-5.5`, `openrouter/anthropic/x`, `ollama/llama3:8b`).

use super::OpenCode;
use crate::agents::{ModelCatalog, ModelTier, SuggestedModel};
use crate::validate::valid_model_shape;

/// Separa il provider dal resto dell'id.
const PROVIDER_SEPARATOR: char = '/';

/// In ordine di preferenza dentro ogni fascia.
const SUGGESTED: &[SuggestedModel] = &[
    SuggestedModel { id: "opencode/gpt-5.4-mini", label: "GPT-5.4 mini (OpenCode Zen)", tier: ModelTier::Standard },
    SuggestedModel { id: "opencode/gpt-5.5", label: "GPT-5.5 (OpenCode Zen)", tier: ModelTier::Strong },
    SuggestedModel { id: "opencode/big-pickle", label: "Big Pickle (OpenCode Zen)", tier: ModelTier::Standard },
];

/// Provider: minuscole, cifre, `.`, `_`, `-`, non inizia con un simbolo.
fn valid_provider(provider: &str) -> bool {
    provider.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && provider.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

impl ModelCatalog for OpenCode {
    fn suggested(&self) -> &'static [SuggestedModel] {
        SUGGESTED
    }

    /// Il modello lo decide la configurazione di OpenCode dell'utente: l'app
    /// non ne passa nessuno.
    fn default_model(&self) -> Option<&'static str> {
        None
    }

    /// `provider/modello`: dentro la forma generica sicura, con un provider
    /// valido e segmenti non vuoti; i `:` (es. `llama3:8b`) solo dopo il provider.
    fn is_valid(&self, model: &str) -> bool {
        valid_model_shape(model)
            && model.split_once(PROVIDER_SEPARATOR).is_some_and(|(provider, rest)| {
                valid_provider(provider) && rest.split(PROVIDER_SEPARATOR).all(|s| !s.is_empty())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_standard_then_strong_zen_models() {
        let ids: Vec<_> = OpenCode.suggested().iter().map(|m| m.id).collect();
        assert_eq!(ids, ["opencode/gpt-5.4-mini", "opencode/gpt-5.5", "opencode/big-pickle"]);
        assert_eq!(OpenCode.suggested()[1].tier, ModelTier::Strong);
        assert_eq!(OpenCode.suggested()[0].label, "GPT-5.4 mini (OpenCode Zen)");
    }

    #[test]
    fn leaves_the_default_to_the_users_config() {
        assert_eq!(OpenCode.default_model(), None);
    }

    #[test]
    fn accepts_provider_slash_model_forms() {
        for ok in [
            "opencode/gpt-5.5",
            "opencode/big-pickle",
            "openrouter/anthropic/x",
            "ollama/llama3:8b",
            "a/b",
            "0x.p_q-1/m",
        ] {
            assert!(OpenCode.is_valid(ok), "{ok}");
        }
    }

    #[test]
    fn rejects_bare_names_bad_providers_and_unsafe_text() {
        for bad in [
            "sonnet", "claude-x", "a:b/c", "/x", "x/", "a//b", "A/b", "-a/b", ".a/b", "a b/c", "a/b c", "a/b;ls",
            "a/b$", "a/b\"", "", "/",
        ] {
            assert!(!OpenCode.is_valid(bad), "{bad:?}");
        }
        assert!(!OpenCode.is_valid(&format!("p/{}", "m".repeat(200))));
    }

    #[test]
    fn suggestions_are_valid() {
        for model in OpenCode.suggested() {
            assert!(OpenCode.is_valid(model.id), "{}", model.id);
        }
    }
}
