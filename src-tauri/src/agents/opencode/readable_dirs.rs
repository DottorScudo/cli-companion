//! Cartelle fuori dalla cartella di lavoro che un worker legge senza conferme
//! (gli allegati dell'Agents HUB del suo workspace), come regole del file di
//! `OPENCODE_CONFIG`: `external_directory(<cartella>/*)` consentita prima
//! delle regole del workspace (la lettura e' gia' consentita dalle regole di
//! modalita') e `edit(<cartella>/*)` vietata dopo tutte, cosi' la cartella
//! resta in sola lettura anche con `--auto`. E' la forma che la documentazione
//! di OpenCode indica per "leggere ma non modificare" una cartella esterna.
//! Puro.
//!
//! Il percorso e' quello che vede OpenCode: nativo su Windows, `/mnt/<d>/...`
//! in WSL. Nella risorsa `*` sostituisce qualsiasi testo, separatori compresi.

use crate::platform::{is_windows_drive_path, windows_to_wsl_path, TargetKind};
use std::path::{Path, PathBuf};

/// Caratteri jolly delle risorse di OpenCode: un percorso che li contiene non
/// diventa una regola (allargherebbe la concessione).
const WILDCARDS: [char; 2] = ['*', '?'];
/// Separatori di percorso di Windows e di WSL.
const SEPARATORS: [char; 2] = ['/', '\\'];

/// La risorsa di ogni cartella di `dirs` che si esprime in modo sicuro; le
/// altre sono saltate (il worker chiedera' il permesso invece di non partire).
pub(super) fn resources(dirs: &[PathBuf], env_kind: TargetKind) -> Vec<String> {
    dirs.iter()
        .filter_map(|dir| match resource(dir, env_kind) {
            Ok(resource) => Some(resource),
            Err(e) => {
                log::warn!("worker: lettura di {} non concessa: {e}", dir.display());
                None
            }
        })
        .collect()
}

/// `<cartella><separatore>*`: tutto cio' che la cartella contiene. Solo una
/// cartella su un drive di Windows, come la cartella dati dell'app.
fn resource(dir: &Path, env_kind: TargetKind) -> Result<String, String> {
    let host = dir.to_string_lossy();
    let mounted = windows_to_wsl_path(&host)
        .filter(|p| is_windows_drive_path(p))
        .ok_or_else(|| format!("not a drive path: {host}"))?;
    let (path, separator) = match env_kind {
        TargetKind::Wsl => (mounted, '/'),
        TargetKind::Windows => (host.to_string(), '\\'),
    };
    let path = path.trim_end_matches(SEPARATORS);
    let relative_component = path.split(SEPARATORS).any(|c| c == "." || c == "..");
    if relative_component || path.chars().any(|c| WILDCARDS.contains(&c) || c.is_control()) {
        return Err(format!("unsafe characters for a permission rule: {path}"));
    }
    Ok(format!("{path}{separator}*"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNEL_DIR: &str = r"C:\Users\Mario Rossi\AppData\Roaming\cli-companion\hub_attachments\channels\ws-1";

    fn dirs(items: &[&str]) -> Vec<PathBuf> {
        items.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn windows_workers_get_the_native_folder() {
        assert_eq!(resources(&dirs(&[CHANNEL_DIR]), TargetKind::Windows), [format!(r"{CHANNEL_DIR}\*")]);
    }

    #[test]
    fn wsl_workers_get_the_mounted_drive() {
        assert_eq!(
            resources(&dirs(&[&format!(r"{CHANNEL_DIR}\")]), TargetKind::Wsl),
            ["/mnt/c/Users/Mario Rossi/AppData/Roaming/cli-companion/hub_attachments/channels/ws-1/*"]
        );
    }

    #[test]
    fn folders_that_cannot_be_a_safe_rule_are_skipped() {
        for bad in ["relative", r"C:\a*b", r"C:\a?b", r"C:\a\..\b", "C:\\a\nb", ""] {
            assert!(resources(&dirs(&[bad]), TargetKind::Windows).is_empty(), "{bad:?}");
        }
        assert!(resources(&dirs(&[r"\\server\share\x"]), TargetKind::Wsl).is_empty());
    }
}
