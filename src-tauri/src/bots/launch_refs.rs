//! Gli avvii predefiniti che puntano a un Bot eliminato. Senza questo passo la
//! voce `bot:<id>` resterebbe appesa e la riconciliazione all'avvio ricreerebbe
//! il Bot derivato. Le funzioni pure lavorano sui dati; `clear_for_deleted_bot`
//! li applica agli store dei workspace e delle impostazioni di avvio.

use super::launches::bot_launch_id;
use crate::launch_settings::{LaunchSettings, LaunchSettingsStore};
use crate::store::data_dir;
use crate::workspaces::{Workspace, WorkspaceStore};
use tauri::{AppHandle, Manager};

/// Azzera (= eredita) l'avvio predefinito dei workspace che apre `launch_id`;
/// `true` se ne ha cambiato almeno uno. Gli altri avvii restano come sono.
fn clear_workspace_defaults(workspaces: &mut [Workspace], launch_id: &str) -> bool {
    let mut changed = false;
    for ws in workspaces.iter_mut().filter(|ws| ws.default_launch.as_deref() == Some(launch_id)) {
        ws.default_launch = None;
        changed = true;
    }
    changed
}

/// Azzera (= shell semplice) l'avvio predefinito globale se e' `launch_id`;
/// `true` se e' cambiato.
fn clear_global_default(settings: &mut LaunchSettings, launch_id: &str) -> bool {
    let matches = settings.default_launch.as_deref() == Some(launch_id);
    if matches {
        settings.default_launch = None;
    }
    matches
}

/// Il Bot `bot_id` e' stato eliminato: i workspace e l'avvio globale che lo
/// usavano come predefinito tornano a ereditare / alla shell semplice. Nessun
/// salvataggio se non c'e' niente da cambiare.
pub(super) fn clear_for_deleted_bot(app: &AppHandle, bot_id: &str) -> Result<(), String> {
    let launch_id = bot_launch_id(bot_id);
    let dir = data_dir(app)?;
    let workspaces = app.state::<WorkspaceStore>();
    if workspaces.all().iter().any(|ws| ws.default_launch.as_deref() == Some(&launch_id)) {
        workspaces.update(&dir, |list| {
            clear_workspace_defaults(list, &launch_id);
            Ok(())
        })?;
    }
    let settings = app.state::<LaunchSettingsStore>();
    if settings.get().default_launch.as_deref() == Some(&launch_id) {
        settings.update(&dir, |s| {
            clear_global_default(s, &launch_id);
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ws(id: &str, default_launch: Option<&str>) -> Workspace {
        serde_json::from_value(json!({
            "id": id, "name": id, "kind": "windows", "path": "C:\\p", "created_at": 1,
            "default_launch": default_launch,
        }))
        .unwrap()
    }

    fn defaults(list: &[Workspace]) -> Vec<Option<&str>> {
        list.iter().map(|w| w.default_launch.as_deref()).collect()
    }

    #[test]
    fn only_the_deleted_bot_is_cleared_from_workspaces() {
        let mut list = vec![
            ws("a", Some("bot:intake-other")),
            ws("b", Some("bot:intake")),
            ws("c", Some("builtin:claude-code")),
            ws("d", Some("ln-1")),
            ws("e", None),
        ];
        assert!(clear_workspace_defaults(&mut list, "bot:intake-other"));
        assert_eq!(defaults(&list), [None, Some("bot:intake"), Some("builtin:claude-code"), Some("ln-1"), None]);
    }

    #[test]
    fn a_prefix_of_the_id_is_not_a_match() {
        let mut list = vec![ws("a", Some("bot:intake-other"))];
        assert!(!clear_workspace_defaults(&mut list, "bot:intake"));
        assert_eq!(defaults(&list), [Some("bot:intake-other")]);
    }

    #[test]
    fn every_workspace_using_the_bot_is_cleared() {
        let mut list = vec![ws("a", Some("bot:setup")), ws("b", Some("bot:setup"))];
        assert!(clear_workspace_defaults(&mut list, "bot:setup"));
        assert_eq!(defaults(&list), [None, None]);
    }

    #[test]
    fn nothing_changes_when_no_workspace_uses_the_bot() {
        let mut list = vec![ws("a", None), ws("b", Some("shell"))];
        assert!(!clear_workspace_defaults(&mut list, "bot:setup"));
        assert_eq!(defaults(&list), [None, Some("shell")]);
        assert!(!clear_workspace_defaults(&mut [], "bot:setup"));
    }

    #[test]
    fn the_global_default_is_cleared_only_for_the_deleted_bot() {
        let mut settings = LaunchSettings { default_launch: Some("bot:setup".into()), show_usage: false };
        assert!(!clear_global_default(&mut settings, "bot:intake"));
        assert_eq!(settings.default_launch.as_deref(), Some("bot:setup"));
        assert!(clear_global_default(&mut settings, "bot:setup"));
        assert_eq!(settings, LaunchSettings { default_launch: None, show_usage: false });
        assert!(!clear_global_default(&mut settings, "bot:setup"));
    }
}
