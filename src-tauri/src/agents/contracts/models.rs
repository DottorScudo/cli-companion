//! Contratto dei modelli: quelli che l'agente consiglia e la regola per
//! riconoscerne il nome.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Fascia di un modello: il core sceglie per fascia, non per nome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ModelTier {
    /// Lavoro ben definito.
    Standard,
    /// Progettazione, modifiche rischiose, bug difficili.
    Strong,
}

/// Un modello consigliato dall'agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SuggestedModel {
    /// Nome come lo vuole l'agente (quello passato al suo comando).
    pub id: &'static str,
    /// Nome mostrato all'utente.
    pub label: &'static str,
    pub tier: ModelTier,
}

/// Modelli di un agente.
pub trait ModelCatalog: Sync {
    /// Modelli consigliati, in ordine di preferenza dentro ogni fascia.
    fn suggested(&self) -> &'static [SuggestedModel];

    /// Modello usato quando non se ne sceglie uno (`None` = decide l'agente).
    fn default_model(&self) -> Option<&'static str>;

    /// Regola dell'integrazione per un nome di modello. Deve rifiutare tutto
    /// cio' che non e' sicuro come singolo argomento di una riga di comando.
    fn is_valid(&self, model: &str) -> bool;
}
