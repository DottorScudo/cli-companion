//! Cartelle fuori dalla cartella di lavoro che un worker legge senza conferme
//! (gli allegati dell'Agents HUB del suo workspace), come regole del file
//! `--settings`: `Read(//<cartella>/**)` fra i consentiti e
//! `Edit(//<cartella>/**)` fra i vietati, cosi' la cartella resta in sola
//! lettura in ogni modalita'. Non `--add-dir` ne'
//! `permissions.additionalDirectories`: una cartella di lavoro aggiuntiva si
//! puo' anche modificare (secondo la modalita') e con `--add-dir` Claude Code
//! ne carica pure la configurazione. Puro.
//!
//! Nelle regole `//` ancora il percorso alla radice del filesystem; su Windows
//! Claude Code confronta i percorsi in forma POSIX (`C:\Users` -> `/c/Users`),
//! in WSL il worker vede `/mnt/c/Users`.

use super::permissions::check_rule;
use crate::platform::{windows_to_wsl_path, TargetKind};
use std::path::{Path, PathBuf};

/// Prefisso dei drive Windows montati in WSL, tolto per la forma POSIX di Windows.
const WSL_MOUNT_ROOT: &str = "/mnt";
/// Caratteri che nelle regole hanno un significato (glob gitignore, parentesi
/// della regola): un percorso che li contiene non diventa una regola.
const PATTERN_CHARS: [char; 7] = ['*', '?', '[', ']', '(', ')', '\\'];

/// Le regole da aggiungere ai consentiti e ai vietati del workspace.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Grants {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

/// Lettura (e mai modifica) di ogni cartella di `dirs`; una cartella che non
/// si esprime in modo sicuro come regola e' saltata: il worker chiedera' il
/// permesso invece di non partire.
pub(super) fn grants(dirs: &[PathBuf], env_kind: TargetKind) -> Grants {
    let mut out = Grants::default();
    for dir in dirs {
        match pattern(dir, env_kind) {
            Ok(pattern) => {
                out.allow.push(format!("Read({pattern})"));
                out.deny.push(format!("Edit({pattern})"));
            }
            Err(e) => log::warn!("worker: lettura di {} non concessa: {e}", dir.display()),
        }
    }
    out
}

/// `//<percorso>/**`: la cartella e tutto cio' che contiene, ancorata alla
/// radice del filesystem dell'ambiente del worker.
fn pattern(dir: &Path, env_kind: TargetKind) -> Result<String, String> {
    let host = dir.to_string_lossy();
    let mounted = windows_to_wsl_path(&host).ok_or_else(|| format!("not an absolute drive path: {host}"))?;
    let posix = match env_kind {
        TargetKind::Wsl => mounted.as_str(),
        TargetKind::Windows => {
            mounted.strip_prefix(WSL_MOUNT_ROOT).ok_or_else(|| format!("not a drive path: {host}"))?
        }
    };
    let posix = posix.trim_end_matches('/');
    let relative_component = posix.split('/').any(|c| c == "." || c == "..");
    if relative_component || posix.chars().any(|c| PATTERN_CHARS.contains(&c) || c.is_control()) {
        return Err(format!("unsafe characters for a permission rule: {posix}"));
    }
    let pattern = format!("/{posix}/**");
    check_rule(&format!("Read({pattern})")).map_err(|code| format!("rule rejected ({code:?}): {pattern}"))?;
    Ok(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNEL_DIR: &str = r"C:\Users\Mario Rossi\AppData\Roaming\cli-companion\hub_attachments\channels\ws-1";

    fn dirs(items: &[&str]) -> Vec<PathBuf> {
        items.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn windows_workers_read_the_posix_form_and_never_edit() {
        let g = grants(&dirs(&[CHANNEL_DIR]), TargetKind::Windows);
        let pattern = "//c/Users/Mario Rossi/AppData/Roaming/cli-companion/hub_attachments/channels/ws-1/**";
        assert_eq!(g, Grants { allow: vec![format!("Read({pattern})")], deny: vec![format!("Edit({pattern})")] });
    }

    #[test]
    fn wsl_workers_read_the_mounted_drive() {
        let g = grants(&dirs(&[CHANNEL_DIR]), TargetKind::Wsl);
        let pattern = "//mnt/c/Users/Mario Rossi/AppData/Roaming/cli-companion/hub_attachments/channels/ws-1/**";
        assert_eq!(g.allow, [format!("Read({pattern})")]);
        assert_eq!(g.deny, [format!("Edit({pattern})")]);
    }

    #[test]
    fn the_grant_covers_only_the_given_folder() {
        let g = grants(&dirs(&[&format!(r"{CHANNEL_DIR}\")]), TargetKind::Windows);
        let rule = &g.allow[0];
        assert!(rule.ends_with("/hub_attachments/channels/ws-1/**)"), "{rule}");
        // Mai la cartella dati intera o il drive.
        assert!(!rule.contains("/cli-companion/**") && !rule.starts_with("Read(//c/**"), "{rule}");
    }

    #[test]
    fn folders_that_cannot_be_a_safe_rule_are_skipped() {
        for bad in [
            r"relative\dir",
            r"\\server\share\x",
            r"C:\a*b\x",
            r"C:\a?b",
            r"C:\Program Files (x86)\x",
            r"C:\a[1]\x",
            r"C:\a\..\b",
            "C:\\a\nb",
            "",
        ] {
            assert_eq!(grants(&dirs(&[bad]), TargetKind::Windows), Grants::default(), "{bad:?}");
        }
        // Una cartella WSL della distro non e' un drive di Windows.
        assert_eq!(grants(&dirs(&[r"\\wsl$\Ubuntu\home\me"]), TargetKind::Windows), Grants::default());
    }

    #[test]
    fn no_folders_means_no_rules() {
        assert_eq!(grants(&[], TargetKind::Wsl), Grants::default());
    }
}
