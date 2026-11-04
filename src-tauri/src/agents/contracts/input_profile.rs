//! Contratto del profilo di input: tasti propri dell'agente nel terminale.

use crate::platform::TargetKind;
use serde::Serialize;

/// Evento di input del terminale che un agente puo' voler gestire in modo
/// diverso dal comportamento standard. PUNTO DI ESTENSIONE: per un nuovo
/// evento si aggiunge una variante (serializzata in kebab-case) e il
/// frontend la richiede con lo stesso nome (`resolveInputOverride`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputEvent {
    /// Incolla con appunti senza testo (es. sola immagine): al posto del
    /// Ctrl+V grezzo si invia la sequenza della regola.
    ImagePaste,
}

/// Regola: per `event`, nel target `env_kind` (`None` = qualunque), il
/// terminale scrive `send` nel pty.
#[derive(Debug, Clone, Serialize)]
pub struct InputRule {
    pub event: InputEvent,
    /// "windows" | "wsl" (vedi `Target::kind`); `None` = tutti i target.
    pub env_kind: Option<TargetKind>,
    pub send: String,
}

/// Profilo di input dichiarativo di un agente.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InputProfile {
    pub rules: Vec<InputRule>,
}

/// Profilo di input dell'agente.
pub trait InputProfileSource: Sync {
    /// Dichiarativo e senza processi: l'app lo legge una volta all'avvio.
    fn input_profile(&self) -> InputProfile;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_input_profile_is_empty() {
        assert!(InputProfile::default().rules.is_empty());
    }
}
