//! Contratto dei permessi dei worker: le regole allow/deny del workspace nel
//! formato che l'agente sa leggere. Il core non conosce la sintassi delle
//! regole: le valida, le scrive e le descrive solo tramite questo contratto.
//! Un'integrazione che non lo dichiara non applica regole.

use crate::platform::TargetKind;
use crate::validate::ValidationError;
use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Forma delle liste di regole accettate dall'agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleFormat {
    /// Regole massime per lista (allow o deny).
    pub max_rules: usize,
    /// Caratteri massimi di una regola.
    pub max_rule_chars: usize,
    /// Una regola di esempio (inglese: la legge il modello nello schema MCP).
    pub example: &'static str,
}

/// Testi per l'interfaccia (italiano), scritti dall'integrazione perche' la
/// sintassi e le avvertenze sono sue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PermissionsGuide {
    /// Come si scrivono le regole e cosa implicano.
    pub rules_help: &'static str,
    /// Avvertenza mostrata quando i worker girano senza conferme (bypass).
    pub bypass_note: &'static str,
    /// Esempi (uno per riga) nei campi vuoti dei Consentiti e dei Vietati.
    pub allow_placeholder: &'static str,
    pub deny_placeholder: &'static str,
}

/// Regole di permesso dei worker.
pub trait WorkerPermissions: Sync {
    fn rule_format(&self) -> RuleFormat;

    fn guide(&self) -> PermissionsGuide;

    /// Valida e normalizza una lista di regole (trim, righe vuote e doppioni
    /// scartati). `Err` tipizzato per l'interfaccia e per il modello.
    fn validate_rules(&self, rules: &[String]) -> Result<Vec<String>, ValidationError>;

    /// Scrive (solo se cambia) le regole del workspace e ritorna cio' che va
    /// in `AutomationRequest::permissions_file`, nel formato di path del
    /// target. `None` = nessuna regola e nessuna cartella da leggere. `Err` se
    /// il risultato non e' usabile in modo sicuro in una riga di comando.
    ///
    /// `readable` sono cartelle dell'host fuori dalla cartella di lavoro che il
    /// worker deve poter leggere senza conferme (gli allegati dell'Agents HUB
    /// del suo workspace): l'integrazione concede la sola lettura di quelle
    /// cartelle e del loro contenuto, mai la scrittura. Una cartella che non sa
    /// esprimere in modo sicuro nel suo formato la salta (il worker chiedera'
    /// il permesso), senza fermare l'avvio.
    fn prepare(
        &self,
        data_dir: &Path,
        workspace_id: &str,
        env_kind: TargetKind,
        allow: &[String],
        deny: &[String],
        readable: &[PathBuf],
    ) -> Result<Option<String>, String>;
}
