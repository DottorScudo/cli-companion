//! Log dell'app: file nella cartella dei log del sistema (con rotazione) e,
//! solo nelle build di debug, anche su stdout.

use log::LevelFilter;
use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind};

/// Nome del file di log (senza estensione).
const LOG_FILE_NAME: &str = "cli-companion";
/// Dimensione oltre la quale il file ruota.
const LOG_MAX_FILE_BYTES: u128 = 1_000_000;
/// File di log ruotati da conservare.
const LOG_FILES_KEPT: usize = 5;

/// Livello minimo: `Info` in release, `Debug` nelle build di sviluppo.
fn log_level() -> LevelFilter {
    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let mut builder = Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir { file_name: Some(LOG_FILE_NAME.into()) }))
        .level(log_level())
        .max_file_size(LOG_MAX_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT));
    if cfg!(debug_assertions) {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }
    builder.build()
}
