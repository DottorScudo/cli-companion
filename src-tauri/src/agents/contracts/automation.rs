//! Contratto di automazione: avviare una sessione senza utente (worker).

use crate::platform::TargetKind;

/// Livello di permessi con cui parte una sessione automatica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Bypass,
}

impl PermissionMode {
    /// Tutti i livelli, dal piu' restrittivo.
    pub const ALL: [PermissionMode; 4] =
        [PermissionMode::Plan, PermissionMode::Default, PermissionMode::AcceptEdits, PermissionMode::Bypass];

    /// Valore ammesso dall'RPC `spawn_session`: "default" | "accept-edits" |
    /// "plan" | "bypass".
    pub fn parse(s: &str) -> Option<PermissionMode> {
        match s {
            "default" => Some(PermissionMode::Default),
            "accept-edits" => Some(PermissionMode::AcceptEdits),
            "plan" => Some(PermissionMode::Plan),
            "bypass" => Some(PermissionMode::Bypass),
            _ => None,
        }
    }

    /// Inverso di `parse`: stesso vocabolario di RPC, impostazioni e UI.
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Plan => "plan",
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "accept-edits",
            PermissionMode::Bypass => "bypass",
        }
    }

    /// Permissivita' crescente: plan < default < accept-edits < bypass.
    pub fn rank(self) -> u8 {
        match self {
            PermissionMode::Plan => 0,
            PermissionMode::Default => 1,
            PermissionMode::AcceptEdits => 2,
            PermissionMode::Bypass => 3,
        }
    }
}

/// Richiesta di avvio di una sessione automatica.
pub struct AutomationRequest<'a> {
    /// ASCII sicuro (`[A-Za-z0-9 .,:;_()'-]`, garantito dal chiamante):
    /// finisce in una riga di comando digitata nella shell del tab.
    pub bootstrap_prompt: &'a str,
    pub permission_mode: PermissionMode,
    /// Gia' validato (`[A-Za-z0-9._-]{1,64}`).
    pub model: Option<&'a str>,
    /// Regole di permesso dei worker preparate dal contratto `WorkerPermissions`
    /// della stessa integrazione, gia' nel formato di path del target.
    pub permissions_file: Option<&'a str>,
}

/// Avvio di sessioni automatiche.
pub trait WorkerAutomation: Sync {
    /// Comando da digitare nella shell del tab per avviare una sessione
    /// automatica. `env_kind` e' la famiglia del target: la stringa deve
    /// funzionare in PowerShell, cmd e bash/zsh. `None` = richiesta non
    /// rappresentabile in modo sicuro (caratteri speciali nel prompt, nel
    /// modello o nel path).
    fn command(&self, env_kind: TargetKind, req: &AutomationRequest) -> Option<String>;
}
