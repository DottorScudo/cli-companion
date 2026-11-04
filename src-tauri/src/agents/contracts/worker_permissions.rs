//! Contratto dei permessi dei worker: le regole allow/deny del workspace nel
//! formato che l'agente sa leggere.

use crate::platform::TargetKind;
use std::path::Path;

/// Preparazione delle regole di permesso dei worker.
pub trait WorkerPermissions: Sync {
    /// Scrive (solo se cambia) le regole del workspace e ritorna cio' che va
    /// in `AutomationRequest::permissions_file`, nel formato di path del
    /// target. `None` = nessuna regola. `Err` se il risultato non e' usabile
    /// in modo sicuro in una riga di comando.
    #[cfg_attr(not(test), expect(dead_code, reason = "wired by a later task (agent-boundaries)"))]
    fn prepare(
        &self,
        data_dir: &Path,
        workspace_id: &str,
        env_kind: TargetKind,
        allow: &[String],
        deny: &[String],
    ) -> Result<Option<String>, String>;
}
