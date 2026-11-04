//! Il tipo che ogni integrazione implementa: identita', rilevamento e l'elenco
//! dei contratti che dichiara.

use super::Capabilities;
use crate::platform::Target;

/// Esito del rilevamento di un agente in un target.
#[derive(Debug, Clone, Default)]
pub struct Detection {
    pub found: bool,
    pub version: Option<String>,
    /// Programma da usare per i comandi successivi (path assoluto su WSL).
    pub path: Option<String>,
}

/// Un coding agent supportato.
pub trait AgentIntegration: Sync {
    /// Id stabile (usato in `--notify`, nelle cartelle e nell'API).
    fn id(&self) -> &'static str;

    fn display_name(&self) -> &'static str;

    /// Rileva l'agente nel target. Puo' lanciare processi: fuori dal main thread.
    fn detect(&self, target: &Target) -> Detection;

    /// Comando che avvia l'agente in modo interattivo. Digitato nella shell
    /// del tab: valido in PowerShell, cmd e bash.
    fn interactive_command(&self) -> &'static str;

    /// Contratti implementati da questa integrazione.
    fn capabilities(&self) -> Capabilities<'_>;
}
