//! Avvii personalizzati di un workspace: aggiunta, modifica, rimozione e avvio
//! predefinito. Le operazioni pure lavorano sul workspace; i comandi Tauri le
//! applicano con "modifica e salva".

use super::model::{Launch, Workspace};
use super::store::{edit, WorkspaceStore, LAUNCH_ID_PREFIX};
use super::validate::{check_default_launch, validate_launch};
use crate::validate::{ErrorCode, ValidationError};
use tauri::{AppHandle, Manager};

/// Aggiunge un avvio validato in coda.
pub(super) fn push_launch(
    launches: &mut Vec<Launch>,
    id: String,
    name: &str,
    command: &str,
) -> Result<(), ValidationError> {
    let (name, command) = validate_launch(name, command)?;
    launches.push(Launch { id, name, command });
    Ok(())
}

/// Cambia nome e comando di un avvio esistente.
pub(super) fn edit_launch(
    launches: &mut [Launch],
    launch_id: &str,
    name: &str,
    command: &str,
) -> Result<(), ValidationError> {
    let (name, command) = validate_launch(name, command)?;
    let launch = launches
        .iter_mut()
        .find(|l| l.id == launch_id)
        .ok_or_else(|| ValidationError::new(ErrorCode::LaunchNotFound))?;
    launch.name = name;
    launch.command = command;
    Ok(())
}

/// Toglie l'avvio `launch_id`; se era il predefinito si torna a ereditare.
pub(super) fn remove_launch_from(ws: &mut Workspace, launch_id: &str) -> Result<(), ValidationError> {
    let before = ws.launches.len();
    ws.launches.retain(|l| l.id != launch_id);
    if ws.launches.len() == before {
        return Err(ValidationError::new(ErrorCode::LaunchNotFound));
    }
    clear_default_if_removed(ws, launch_id);
    Ok(())
}

/// Gli avvii che ogni workspace puo' scegliere come predefinito oltre ai suoi:
/// gli integrati e le voci dei Bot abilitati (`bot_launches`).
pub fn shared_launch_ids(bot_launches: Vec<String>) -> Vec<String> {
    crate::agents::builtin_launch_list().into_iter().map(|b| b.id).chain(bot_launches).collect()
}

/// L'avvio `removed` non c'e' piu': se era il predefinito, si torna a ereditare.
pub(super) fn clear_default_if_removed(ws: &mut Workspace, removed: &str) {
    if ws.default_launch.as_deref() == Some(removed) {
        ws.default_launch = None;
    }
}

#[tauri::command]
pub async fn add_launch(
    app: AppHandle,
    workspace_id: String,
    name: String,
    command: String,
    is_default: bool,
) -> Result<Workspace, String> {
    crate::blocking(move || {
        let id = app.state::<WorkspaceStore>().next_id(LAUNCH_ID_PREFIX);
        edit(&app, &workspace_id, |ws| {
            push_launch(&mut ws.launches, id.clone(), &name, &command)?;
            if is_default {
                ws.default_launch = Some(id);
            }
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn update_launch(
    app: AppHandle,
    workspace_id: String,
    launch_id: String,
    name: String,
    command: String,
) -> Result<Workspace, String> {
    crate::blocking(move || {
        edit(&app, &workspace_id, |ws| {
            edit_launch(&mut ws.launches, &launch_id, &name, &command)?;
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn remove_launch(app: AppHandle, workspace_id: String, launch_id: String) -> Result<Workspace, String> {
    crate::blocking(move || {
        edit(&app, &workspace_id, |ws| {
            remove_launch_from(ws, &launch_id)?;
            Ok(())
        })
    })
    .await
}

/// Imposta l'avvio predefinito del workspace (None = eredita il globale).
#[tauri::command]
pub async fn set_default_launch(
    app: AppHandle,
    workspace_id: String,
    launch_id: Option<String>,
) -> Result<Workspace, String> {
    crate::blocking(move || {
        let shared = shared_launch_ids(crate::bots::enabled_launch_ids(&app));
        edit(&app, &workspace_id, |ws| {
            if let Some(id) = launch_id.as_deref() {
                check_default_launch(&ws.launches, id, &shared)?;
            }
            ws.default_launch = launch_id;
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::TargetKind;

    fn launch(id: &str) -> Launch {
        Launch { id: id.to_string(), name: id.to_string(), command: "ls".to_string() }
    }

    fn ws_with(launches: Vec<Launch>) -> Workspace {
        let mut ws = Workspace::new("ws-1".into(), "p".into(), TargetKind::Windows, "C:\\p".into());
        ws.launches = launches;
        ws
    }

    #[test]
    fn push_and_edit_launches() {
        let mut v = Vec::new();
        push_launch(&mut v, "a".into(), "A", "ls").unwrap();
        push_launch(&mut v, "b".into(), "B", "pwd").unwrap();
        edit_launch(&mut v, "a", "A2", "dir").unwrap();
        assert_eq!((v[0].name.as_str(), v[0].command.as_str()), ("A2", "dir"));
        assert_eq!(edit_launch(&mut v, "zzz", "X", "ls").unwrap_err().code(), ErrorCode::LaunchNotFound);
        // Input non valido: nessuna modifica.
        assert_eq!(push_launch(&mut v, "c".into(), "", "ls").unwrap_err().code(), ErrorCode::LaunchNameMissing);
        assert_eq!(edit_launch(&mut v, "a", "ok", " ").unwrap_err().code(), ErrorCode::LaunchCommandMissing);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "A2");
    }

    #[test]
    fn removing_the_default_launch_goes_back_to_inheriting() {
        let mut ws = ws_with(vec![launch("l1"), launch("l2")]);
        ws.default_launch = Some("l1".into());
        remove_launch_from(&mut ws, "l2").unwrap();
        assert_eq!(ws.default_launch.as_deref(), Some("l1"), "un altro avvio non tocca il predefinito");
        remove_launch_from(&mut ws, "l1").unwrap();
        assert!(ws.launches.is_empty());
        assert_eq!(ws.default_launch, None);
    }

    #[test]
    fn removing_an_unknown_launch_fails_and_changes_nothing() {
        let mut ws = ws_with(vec![launch("l1")]);
        ws.default_launch = Some("l1".into());
        assert_eq!(remove_launch_from(&mut ws, "nope").unwrap_err().code(), ErrorCode::LaunchNotFound);
        assert_eq!(ws.launches.len(), 1);
        assert_eq!(ws.default_launch.as_deref(), Some("l1"));
    }

    #[test]
    fn a_builtin_or_shell_default_survives_the_removal_of_a_launch() {
        let mut ws = ws_with(vec![launch("l1")]);
        ws.default_launch = Some("shell".into());
        remove_launch_from(&mut ws, "l1").unwrap();
        assert_eq!(ws.default_launch.as_deref(), Some("shell"));
    }
}
