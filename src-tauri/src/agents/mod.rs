//! Integrazioni dei coding agent. Il core non sa come funziona un agente: offre
//! dei contratti (`contracts/`, uno per capacita') e ogni integrazione
//! implementa quelli che le servono.
//!
//! Aggiungere un agente:
//! 1. una cartella `agents/<nome>/` con una struct che implementa
//!    `AgentIntegration` (id, nome, rilevamento, comando interattivo) e i
//!    contratti che le servono, dichiarati TUTTI (presenti o `None`) in
//!    `capabilities()` (vedi `claude_code/`);
//! 2. una riga in `INTEGRATIONS` di `registry.rs`.
//!
//! Comandi Tauri e UI (Impostazioni -> Coding agent) sono generici e non vanno
//! toccati; `conformance.rs` verifica da solo ogni integrazione registrata.

mod claude_code;
mod commands;
#[cfg(test)]
mod conformance;
mod contracts;
mod detection;
mod launches;
mod package;
pub mod presets;
pub mod registry;
#[cfg(test)]
mod testing;

/// Id e cartella di configurazione di Claude Code: le skill utente (`user_skills`)
/// li usano ancora direttamente, fino al task agent-boundaries.
pub(crate) use claude_code::{windows_claude_dir, AGENT_ID as CLAUDE_CODE_ID};
pub use commands::*;
pub use contracts::*;
pub use launches::{builtin_launch_list, BuiltinLaunch};
pub use registry::{automation_command, DEFAULT_WORKER_AGENT};
