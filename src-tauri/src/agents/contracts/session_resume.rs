//! Contratto di ripristino: riprendere la conversazione di una sessione dopo
//! il riavvio dell'app. Solo dati: li interpreta il frontend, che cosi' non
//! sa nulla di come un agente riprende una sessione.

use serde::Serialize;

/// Segnaposto del comando originale (senza i flag di ripresa) nel modello. Il
/// core non sostituisce nulla (lo fa il frontend): serve alla conformita'.
#[cfg(test)]
pub const COMMAND_PLACEHOLDER: &str = "{command}";
/// Segnaposto dell'id di sessione nel modello.
#[cfg(test)]
pub const SESSION_ID_PLACEHOLDER: &str = "{id}";

/// Un flag che l'agente usa per riprendere o continuare una sessione.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumeFlag {
    /// Forme equivalenti del flag (es. lunga e corta), ciascuna con il trattino.
    pub names: Vec<&'static str>,
    /// Il flag accetta un valore (`--flag x` o `--flag=x`).
    pub takes_value: bool,
}

/// Dati con cui il frontend riprende una sessione dopo il riavvio.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumeSpec {
    /// Nomi del programma (primo token del comando, senza distinzione di
    /// maiuscole). Il primo e' quello canonico: si usa quando il tab non sta
    /// eseguendo l'agente.
    pub command_names: Vec<&'static str>,
    /// Flag di ripresa da togliere dal comando originale prima di accodare
    /// quello nuovo.
    pub strip_flags: Vec<ResumeFlag>,
    /// Modello del comando di ripresa, con i segnaposto `COMMAND_PLACEHOLDER`
    /// e `SESSION_ID_PLACEHOLDER`.
    pub resume_template: &'static str,
}

/// Come l'agente riprende una sessione precedente.
pub trait SessionResume: Sync {
    /// Dati con cui il frontend riprende una sessione dopo il riavvio.
    fn resume_spec(&self) -> ResumeSpec;
}
