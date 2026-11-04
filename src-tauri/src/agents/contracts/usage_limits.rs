//! Contratto dei limiti dell'abbonamento.

use crate::platform::Target;
use serde::Serialize;

/// Una finestra di limite dell'abbonamento (5 ore o settimanale).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LimitWindow {
    /// Percentuale consumata (0..=100, puo' superare 100 in sforamento).
    pub used_percentage: f64,
    /// Fine della finestra, RFC3339 come ricevuto.
    pub resets_at: Option<String>,
}

/// Limiti dell'abbonamento e loro stato.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageLimitsReply {
    /// "ok" | "expired" | "unavailable" | "disabled" | "unsupported".
    pub state: String,
    pub five_hour: Option<LimitWindow>,
    pub seven_day: Option<LimitWindow>,
    /// Spiegazione breve (italiano) quando lo stato non e' "ok".
    pub message: Option<String>,
}

impl UsageLimitsReply {
    /// Risposta senza finestre, con stato e (opzionale) messaggio.
    pub fn status(state: &str, message: Option<&str>) -> Self {
        Self { state: state.to_string(), five_hour: None, seven_day: None, message: message.map(str::to_string) }
    }
}

/// Lettura dei limiti dell'abbonamento.
pub trait UsageLimits: Sync {
    /// Limiti nel target. Bloccante (rete): da chiamare fuori dal main thread.
    fn limits(&self, target: &Target) -> UsageLimitsReply;
}
