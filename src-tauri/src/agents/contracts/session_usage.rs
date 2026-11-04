//! Contratto dei consumi di sessione: token letti dal transcript.

use crate::platform::Target;
use serde::Serialize;
use ts_rs::TS;

/// Token contati in una sessione.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

/// Consumi di una sessione letti dal transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SessionUsage {
    pub main: TokenTotals,
    pub subagents: TokenTotals,
    /// input + cache read + cache creation dell'ultimo messaggio della catena
    /// principale: la dimensione attuale del contesto.
    pub context_tokens: u64,
    /// Modello dell'ultimo messaggio principale.
    pub model: Option<String>,
    /// Messaggi distinti della catena principale.
    pub messages: u32,
}

/// Lettura dei consumi di una sessione.
pub trait SessionUsageReader: Sync {
    /// Consumi della sessione col transcript `transcript_path`, come path
    /// visto dall'agente nel target. Una eventuale cache e' interna
    /// all'integrazione. Bloccante (disco): fuori dal main thread.
    fn session_usage(&self, target: &Target, transcript_path: &str) -> Result<SessionUsage, String>;
}
