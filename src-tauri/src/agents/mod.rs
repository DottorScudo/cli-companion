//! Integrazioni dei coding agent. Il core non sa come funziona un agente: offre
//! dei contratti (`contracts/`, uno per capacita') e ogni integrazione
//! implementa quelli che le servono.
//!
//! Aggiungere un agente:
//! 1. una cartella `agents/<nome>/` con una struct che implementa
//!    `AgentIntegration` (id, nome, rilevamento, comando interattivo) e i
//!    contratti che le servono, dichiarati TUTTI (presenti o `None`) in
//!    `capabilities()` (vedi `claude_code/`);
//! 2. una riga in `INTEGRATIONS` di `registry.rs` e una in quello di
//!    `architecture.rs`.
//!
//! Comandi Tauri e UI (Impostazioni -> Coding agent) sono generici e non vanno
//! toccati; `conformance.rs` verifica da solo ogni integrazione registrata e
//! `architecture.rs` che il nome di un agente non compaia fuori dalla sua
//! cartella. Guida completa: `docs/wiki/integrare-un-coding-agent.md`.

#[cfg(test)]
mod architecture;
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
pub(crate) mod testing;
mod worker_rules;

pub use commands::*;
pub use contracts::*;
pub use launches::{builtin_launch_list, BuiltinLaunch};
pub use registry::{automation_command, default_worker_integration};
pub use worker_rules::{validate_worker_rules, worker_permissions_file, worker_rule_format};
