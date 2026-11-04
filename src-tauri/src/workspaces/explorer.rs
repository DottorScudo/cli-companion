//! Apre la cartella di un workspace (o un file) in Esplora risorse.

use super::model::Workspace;
use super::store::{WorkspaceStore, WORKSPACE_NOT_FOUND};
use crate::platform::{hide_window, Target};
use tauri::{AppHandle, Manager};

/// Percorso UNC di una cartella WSL visibile da Windows:
/// (`Ubuntu`, `/home/me/proj`) -> `\\wsl.localhost\Ubuntu\home\me\proj`.
fn wsl_unc_path(distro: &str, path: &str) -> String {
    let rest = path.trim_matches('/').replace('/', "\\");
    if rest.is_empty() {
        format!("\\\\wsl.localhost\\{distro}")
    } else {
        format!("\\\\wsl.localhost\\{distro}\\{rest}")
    }
}

/// Percorso da passare a explorer.exe per il workspace.
fn explorer_target(ws: &Workspace) -> Result<String, String> {
    Ok(match ws.target()? {
        Target::Windows => ws.path.clone(),
        Target::Wsl { distro } => wsl_unc_path(&distro, &ws.path),
    })
}

/// Apre una cartella o un file con explorer.exe (un file si apre con
/// l'applicazione predefinita).
pub(crate) fn open_in_explorer(target: &std::ffi::OsStr) -> Result<(), String> {
    let mut cmd = std::process::Command::new("explorer.exe");
    cmd.arg(target);
    hide_window(&mut cmd);
    // explorer.exe esce spesso con codice 1 anche se la cartella si apre:
    // conta solo il fallimento dello spawn, l'exit code non si guarda.
    cmd.spawn().map(|_| ()).map_err(|e| format!("impossibile aprire Esplora risorse: {e}"))
}

/// Async per coerenza con gli altri comandi che lanciano processi.
#[tauri::command]
pub async fn open_workspace_folder(app: AppHandle, id: String) -> Result<(), String> {
    crate::blocking(move || {
        let ws = app.state::<WorkspaceStore>().find(&id).ok_or_else(|| WORKSPACE_NOT_FOUND.to_string())?;
        open_in_explorer(explorer_target(&ws)?.as_ref())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::TargetKind;

    #[test]
    fn wsl_unc_path_uses_backslashes() {
        assert_eq!(wsl_unc_path("Ubuntu", "/home/me/proj"), "\\\\wsl.localhost\\Ubuntu\\home\\me\\proj");
        assert_eq!(wsl_unc_path("Ubuntu", "/"), "\\\\wsl.localhost\\Ubuntu");
        assert_eq!(wsl_unc_path("Ubuntu-24.04", "/home/me/proj/"), "\\\\wsl.localhost\\Ubuntu-24.04\\home\\me\\proj");
    }

    #[test]
    fn the_target_depends_on_the_workspace_kind() {
        let windows = Workspace::new("a".into(), "a".into(), TargetKind::Windows, "C:\\proj".into());
        assert_eq!(explorer_target(&windows).unwrap(), "C:\\proj");
        let mut wsl = Workspace::new("b".into(), "b".into(), TargetKind::Wsl, "/home/me".into());
        wsl.distro = Some("Ubuntu".into());
        assert_eq!(explorer_target(&wsl).unwrap(), "\\\\wsl.localhost\\Ubuntu\\home\\me");
        wsl.distro = None;
        assert!(explorer_target(&wsl).is_err(), "WSL senza distro non ha un percorso");
    }
}
