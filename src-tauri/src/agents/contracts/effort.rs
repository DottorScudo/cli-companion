//! Contratto dell'effort: il livello di ragionamento che l'agente dedica a una
//! sessione e come si chiede nella sua riga di comando.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Livello di ragionamento del modello. Il vocabolario e' del core: ogni agente
/// dichiara quali livelli sa applicare (`EffortLevels::levels`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    /// Tutti i livelli, dal piu' basso.
    pub const ALL: [Effort; 5] = [Effort::Low, Effort::Medium, Effort::High, Effort::XHigh, Effort::Max];

    /// Testo sul filo e nelle righe di comando: lo stesso di serde.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

/// Effort di un agente.
pub trait EffortLevels: Sync {
    /// Livelli che l'agente sa applicare, dal piu' basso (mai vuoto).
    fn levels(&self) -> &'static [Effort];

    /// Argomenti da aggiungere alla riga di comando per il livello `effort`.
    /// Chiamato solo con un livello di `levels`; `None` = non rappresentabile.
    fn command_args(&self, effort: Effort) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_text_matches_serde_for_every_level() {
        for effort in Effort::ALL {
            assert_eq!(serde_json::to_value(effort).unwrap(), effort.as_str());
            let parsed: Effort = serde_json::from_value(effort.as_str().into()).unwrap();
            assert_eq!(parsed, effort);
        }
    }

    #[test]
    fn effort_rejects_values_outside_the_enum() {
        for bad in ["", "LOW", "ultra", "high --x", "x-high"] {
            assert!(serde_json::from_value::<Effort>(bad.into()).is_err(), "{bad:?}");
        }
    }
}
