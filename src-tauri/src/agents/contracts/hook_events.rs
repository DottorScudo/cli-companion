//! Contratto degli eventi hook: come l'agente avvisa l'app (sessione avviata,
//! finita, in attesa di una risposta) e che dettagli porta con se'.

use serde::Serialize;

/// Evento che l'hook dell'agente segnala con `cli-companion --notify <evento> <agente>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookEventKind {
    SessionStart,
    Finished,
    SessionEnd,
    NeedsInput,
    PermissionRequest,
}

impl HookEventKind {
    /// Tutti i tipi, per le suite di conformita' e di protocollo.
    #[cfg(test)]
    pub const ALL: [HookEventKind; 5] = [
        HookEventKind::SessionStart,
        HookEventKind::Finished,
        HookEventKind::SessionEnd,
        HookEventKind::NeedsInput,
        HookEventKind::PermissionRequest,
    ];
}

/// Dettagli di un evento in forma neutra, indipendente dall'agente. Ogni
/// campo e' opzionale: l'agente fornisce quelli che conosce.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookEvent {
    pub agent_session_id: Option<String>,
    pub notification_type: Option<String>,
    pub message: Option<String>,
    /// La sessione sta gia' proseguendo dopo uno stop bloccato.
    pub already_continuing: Option<bool>,
    /// Lavori in background ancora attivi.
    pub background_work: Option<u32>,
    pub start_source: Option<String>,
    pub end_reason: Option<String>,
    pub transcript_path: Option<String>,
    pub tool_name: Option<String>,
    /// Riassunto breve dell'input del tool (comando, path, URL).
    pub tool_summary: Option<String>,
}

/// Lettura degli eventi hook dell'agente.
pub trait HookEvents: Sync {
    /// Normalizza lo stdin grezzo dell'hook. Tollerante: un input malformato
    /// da' un evento senza dettagli, mai un errore.
    fn normalize(&self, kind: HookEventKind, raw: &str) -> HookEvent;

    /// Riga da scrivere su stdout dell'hook di fine turno perche' l'agente
    /// prosegua invece di fermarsi.
    fn stop_block_output(&self, reason: &str) -> String;
}
