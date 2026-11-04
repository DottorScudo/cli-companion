//! Migrazione dei dati delle vecchie installazioni: la cartella dati dipende
//! dall'identifier dell'app, quindi al primo avvio dopo un cambio di nome se
//! ne copiano i file.

use std::io;
use std::path::Path;
use tauri::Manager;

/// Identifier della vecchia app (muse-companion): la cartella dati dipende
/// dall'identifier, quindi al primo avvio se ne copiano i file.
const LEGACY_IDENTIFIER: &str = "com.jmagarelli.muse-companion";
const DATA_FILES: [&str; 1] = ["workspaces.json"];
/// Identifier di cli-companion fino alla v0.9.1: la sua cartella dati viene
/// copiata per intero nella nuova.
const PREVIOUS_IDENTIFIER: &str = "com.jmagarelli.cli-companion";

/// Copia ricorsiva di una cartella (i file gia' presenti in `to` sono sovrascritti).
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Copia i dati delle vecchie installazioni se la cartella dati nuova non li
/// ha ancora. Le cartelle legacy restano intatte: il plugin dell'agente gia'
/// installato punta ancora a quella vecchia finche' non viene reinstallato.
pub(crate) fn migrate_legacy_data(app: &tauri::App) {
    match app.path().app_data_dir() {
        Ok(dir) => migrate_legacy_at(&dir),
        Err(e) => log::error!("cartella dati non disponibile, migrazione dei dati legacy saltata: {e}"),
    }
}

/// Logica pura di `migrate_legacy_data`: `dir` e' la cartella dati nuova, le
/// vecchie sono sue cartelle sorelle (una per identifier).
pub(crate) fn migrate_legacy_at(dir: &Path) {
    if let Some(previous) = dir.parent().map(|p| p.join(PREVIOUS_IDENTIFIER)) {
        if previous != dir && previous.is_dir() && !dir.exists() {
            match copy_dir(&previous, dir) {
                Ok(()) => log::info!("dati migrati da {}", previous.display()),
                Err(e) => log::error!("migrazione dati da {}: {e}", previous.display()),
            }
        }
    }
    let Some(legacy) = dir.parent().map(|p| p.join(LEGACY_IDENTIFIER)) else {
        return;
    };
    if legacy == dir || !legacy.is_dir() {
        return;
    }
    for name in DATA_FILES {
        let (from, to) = (legacy.join(name), dir.join(name));
        if !from.is_file() || to.exists() {
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::warn!("cartella dati {}: {e}", dir.display());
            continue;
        }
        match std::fs::copy(&from, &to) {
            Ok(_) => log::info!("{name} copiato da {}", legacy.display()),
            Err(e) => log::warn!("copia di {name} da {}: {e}", legacy.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-legacy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn previous_folder_is_copied_whole_when_target_is_missing() {
        let root = temp_root("previous");
        let previous = root.join(PREVIOUS_IDENTIFIER);
        std::fs::create_dir_all(previous.join("boards")).unwrap();
        std::fs::write(previous.join("workspaces.json"), "ws").unwrap();
        std::fs::write(previous.join("boards").join("w1.json"), "board").unwrap();
        let target = root.join("target-dir");
        migrate_legacy_at(&target);
        assert_eq!(std::fs::read_to_string(target.join("workspaces.json")).unwrap(), "ws");
        assert_eq!(std::fs::read_to_string(target.join("boards").join("w1.json")).unwrap(), "board");
        // L'originale resta intatto.
        assert_eq!(std::fs::read_to_string(previous.join("workspaces.json")).unwrap(), "ws");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_is_copied_when_target_exists() {
        let root = temp_root("exists");
        let previous = root.join(PREVIOUS_IDENTIFIER);
        std::fs::create_dir_all(&previous).unwrap();
        std::fs::write(previous.join("workspaces.json"), "ws").unwrap();
        std::fs::write(previous.join("altro.json"), "altro").unwrap();
        let target = root.join("target-dir");
        std::fs::create_dir_all(&target).unwrap();
        migrate_legacy_at(&target);
        assert!(!target.join("workspaces.json").exists());
        assert!(!target.join("altro.json").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_workspaces_are_copied_only_when_missing() {
        let root = temp_root("legacy");
        let legacy = root.join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("workspaces.json"), "vecchio").unwrap();
        std::fs::write(legacy.join("altro.json"), "altro").unwrap();
        let target = root.join("target-dir");
        migrate_legacy_at(&target);
        assert_eq!(std::fs::read_to_string(target.join("workspaces.json")).unwrap(), "vecchio");
        assert!(!target.join("altro.json").exists(), "solo i file in DATA_FILES");
        // Un secondo giro non sovrascrive il file nuovo.
        std::fs::write(target.join("workspaces.json"), "nuovo").unwrap();
        migrate_legacy_at(&target);
        assert_eq!(std::fs::read_to_string(target.join("workspaces.json")).unwrap(), "nuovo");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_folder_equal_to_target_is_a_noop() {
        let root = temp_root("same");
        let dir = root.join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("workspaces.json"), "ws").unwrap();
        migrate_legacy_at(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("workspaces.json")).unwrap(), "ws");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn previous_folder_equal_to_target_is_a_noop() {
        let root = temp_root("same-previous");
        let dir = root.join(PREVIOUS_IDENTIFIER);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.json"), "x").unwrap();
        migrate_legacy_at(&dir);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_legacy_folders_changes_nothing() {
        let root = temp_root("none");
        let target = root.join("target-dir");
        migrate_legacy_at(&target);
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
