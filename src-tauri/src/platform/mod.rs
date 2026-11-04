//! Dove gira un comando (Windows o una distro WSL) e come si lancia.
//!
//! Tutta la conoscenza di piattaforma sta qui: il tipo `Target` (e la sua
//! famiglia `TargetKind`, persistita nei workspace), i costruttori dei
//! comandi (shim npm via `cmd /c`, login script WSL, finestra nascosta),
//! l'esecuzione con timeout e output limitato, le conversioni di path
//! Windows/WSL. Gli altri moduli usano questi tipi invece di confrontare
//! stringhe o costruire `wsl.exe` a mano.

mod command;
mod exec;
mod paths;
mod target;

pub(crate) use command::{hide_window, wsl_exe, wsl_run_command, WSL_EXE};
pub(crate) use exec::output_with_timeout;
pub(crate) use paths::is_valid_distro_name;
pub use paths::{forward_slashes, is_windows_drive_path, windows_to_wsl_path, wsl_to_windows_path};
pub use target::{Target, TargetKind, WINDOWS_KEY};
