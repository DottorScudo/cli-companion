//! I comandi Tauri dei connettori. Validano gli argomenti per primi (`checks`,
//! pura), fanno l'I/O in `crate::blocking` e rispondono in italiano. Nessun
//! comando restituisce o scrive nel log un segreto: dei segreti si vede solo
//! se uno slot e' impostato.
//!
//! - `instances`: elenco, creazione, modifica, abilitazione, eliminazione e
//!   segreti delle istanze, descrittori e stato di salute;
//! - `actions`: prova e azioni dichiarate (consenso, abbinamento);
//! - `routes`: regole delle fonti e collegamenti dei canali di contatto;
//! - `checks`: le verifiche pure; `view`: i DTO.

pub mod actions;
pub mod checks;
pub mod instances;
pub mod routes;
#[cfg(test)]
mod support;
pub mod view;
