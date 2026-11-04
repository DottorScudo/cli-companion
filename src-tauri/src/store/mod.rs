//! Persistenza JSON comune a tutti i file di impostazioni dell'app.
//!
//! Formato su disco (`<app_data>/<nome>.json`, JSON indentato):
//!
//! ```json
//! { "schema_version": 1, "data": { ... } }
//! ```
//!
//! - **Versione**: la versione corrente di un file e' il numero dei suoi
//!   passi di migrazione (`steps.len()`); aggiungere un passo la incrementa.
//! - **File senza busta** (formato storico): sono la versione 0, l'intero
//!   JSON e' il dato. Il passo 0 -> 1 introduce la busta (`adopt_envelope`).
//! - **Migrazioni**: lista ordinata di funzioni pure `Value -> Value`
//!   (`migrate`), applicate in sequenza dalla versione del file a quella
//!   corrente.
//! - **Nessuna perdita silenziosa**: un file corrotto, non migrabile o scritto
//!   da una versione piu' recente non viene mai cancellato ne' sovrascritto;
//!   viene rinominato in `<nome>.bak-<AAAAMMGG-OOMMSSmmm>` (UTC, se ne tengono
//!   gli ultimi 5) e si riparte dai valori predefiniti. Se il file non si puo'
//!   nemmeno leggere (permessi, file bloccato) resta dov'e' e i salvataggi di
//!   quel file vengono rifiutati.
//! - **Voci escluse**: un file che e' un elenco di voci indipendenti puo' usare
//!   `JsonFile::decoded_with`: le voci illeggibili vengono escluse, le altre
//!   caricate, e il file originale copiato in un backup datato (avviso
//!   `PartiallyRead`).
//! - **Avvisi**: ogni anomalia produce uno `StoreWarning`, registrato nel log,
//!   visibile con il comando `store_warnings` e notificato al frontend con
//!   l'evento `store-warning`.
//! - **Nome a runtime e lettura da testo**: `JsonFile::named` crea un file il
//!   cui nome si conosce solo a runtime (una istanza per file, con il proprio
//!   blocco delle scritture); `JsonFile::parse` applica busta, migrazioni e
//!   validazione a un testo in memoria, senza I/O.
//! - **Scrittura**:`write_atomic` (file temporaneo sincronizzato + rename).

mod atomic;
mod backup;
mod envelope;
mod file;
mod json_store;
pub(crate) mod legacy;
mod migrate;
pub(crate) mod warnings;

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) use atomic::write_atomic;
pub use file::{Decoded, JsonFile, Loaded};
pub use json_store::JsonStore;
pub use migrate::{adopt_envelope, Migration};
pub use warnings::{install_emitter, StoreWarning, WarningKind};

/// Cartella dati dell'app (`app_data_dir` di Tauri).
pub(crate) fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}
