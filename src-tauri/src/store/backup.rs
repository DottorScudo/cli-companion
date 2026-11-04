//! Backup datati dei file messi da parte (corrotti o di una versione piu'
//! recente): mai cancellati subito, se ne tengono gli ultimi `BACKUPS_KEPT`.

use chrono::{DateTime, Duration, Utc};
use std::io;
use std::path::Path;

/// Backup tenuti per ciascun file.
const BACKUPS_KEPT: usize = 5;
/// Formato del timestamp nel nome: UTC, ordinabile lessicograficamente.
const STAMP_FORMAT: &str = "%Y%m%d-%H%M%S%3f";

fn backup_prefix(name: &str) -> String {
    format!("{name}.bak-")
}

/// `<name>.bak-<AAAAMMGG-OOMMSSmmm>` (UTC).
fn backup_name(name: &str, now: DateTime<Utc>) -> String {
    format!("{}{}", backup_prefix(name), now.format(STAMP_FORMAT))
}

/// Rinomina `dir/name` in un backup datato e ritorna il NOME del backup, poi
/// pota i backup piu' vecchi dello stesso file. Il file non viene mai
/// cancellato; se esiste gia' un backup con lo stesso istante si avanza di un
/// millisecondo per non sovrascriverlo.
pub(crate) fn quarantine(dir: &Path, name: &str, now: DateTime<Utc>) -> io::Result<String> {
    let backup = free_backup_name(dir, name, now);
    std::fs::rename(dir.join(name), dir.join(&backup))?;
    prune(dir, name, BACKUPS_KEPT);
    Ok(backup)
}

/// Come `quarantine`, ma il file originale resta dov'e': serve quando se ne
/// usa ancora una parte (voci illeggibili escluse, il resto caricato).
pub(crate) fn preserve(dir: &Path, name: &str, now: DateTime<Utc>) -> io::Result<String> {
    let backup = free_backup_name(dir, name, now);
    std::fs::copy(dir.join(name), dir.join(&backup))?;
    prune(dir, name, BACKUPS_KEPT);
    Ok(backup)
}

/// Nome di backup per `now`, avanzando di un millisecondo finche' e' occupato.
fn free_backup_name(dir: &Path, name: &str, now: DateTime<Utc>) -> String {
    let mut at = now;
    while dir.join(backup_name(name, at)).exists() {
        at += Duration::milliseconds(1);
    }
    backup_name(name, at)
}

/// Tiene i `keep` backup piu' recenti di `name` (ordinamento per nome, cioe'
/// per data) ed elimina gli altri. Gli errori di eliminazione sono solo
/// registrati. I vecchi `*.json.bak` non corrispondono e restano dove sono.
fn prune(dir: &Path, name: &str, keep: usize) {
    let prefix = backup_prefix(name);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix))
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(keep);
    for old in &backups[..excess] {
        if let Err(e) = std::fs::remove_file(dir.join(old)) {
            log::warn!("impossibile eliminare il vecchio backup {old}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn at(ms: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap() + Duration::milliseconds(ms)
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn name_is_utc_with_milliseconds_and_sortable() {
        assert_eq!(backup_name("a.json", at(42)), "a.json.bak-20260304-050607042");
        assert!(backup_name("a.json", at(0)) < backup_name("a.json", at(1)));
        assert!(backup_name("a.json", at(999)) < backup_name("a.json", at(1000)));
    }

    #[test]
    fn quarantine_moves_the_file_and_keeps_its_content() {
        let d = temp_dir("move");
        std::fs::write(d.join("a.json"), "contenuto").unwrap();
        let backup = quarantine(&d, "a.json", at(0)).unwrap();
        assert_eq!(backup, "a.json.bak-20260304-050607000");
        assert!(!d.join("a.json").exists());
        assert_eq!(std::fs::read_to_string(d.join(&backup)).unwrap(), "contenuto");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn preserve_copies_and_leaves_the_original() {
        let d = temp_dir("copy");
        std::fs::write(d.join("a.json"), "contenuto").unwrap();
        let backup = preserve(&d, "a.json", at(0)).unwrap();
        assert_eq!(backup, "a.json.bak-20260304-050607000");
        assert_eq!(std::fs::read_to_string(d.join("a.json")).unwrap(), "contenuto");
        assert_eq!(std::fs::read_to_string(d.join(&backup)).unwrap(), "contenuto");
        let second = preserve(&d, "a.json", at(0)).unwrap();
        assert_ne!(backup, second, "same instant never overwrites a backup");
        assert!(preserve(&d, "missing.json", at(0)).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn same_instant_never_overwrites_a_backup() {
        let d = temp_dir("collision");
        std::fs::write(d.join("a.json"), "uno").unwrap();
        let first = quarantine(&d, "a.json", at(0)).unwrap();
        std::fs::write(d.join("a.json"), "due").unwrap();
        let second = quarantine(&d, "a.json", at(0)).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(d.join(first)).unwrap(), "uno");
        assert_eq!(std::fs::read_to_string(d.join(second)).unwrap(), "due");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_file_is_an_error() {
        let d = temp_dir("missing");
        assert!(quarantine(&d, "a.json", at(0)).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn prune_keeps_the_newest_of_that_file_only() {
        let d = temp_dir("prune");
        for i in 0..8 {
            std::fs::write(d.join(backup_name("a.json", at(i))), "x").unwrap();
        }
        std::fs::write(d.join(backup_name("b.json", at(0))), "x").unwrap();
        std::fs::write(d.join("a.json.bak"), "vecchio").unwrap();
        std::fs::write(d.join("a.json"), "attivo").unwrap();
        prune(&d, "a.json", 5);
        let left = files(&d);
        let a_backups: Vec<&String> = left.iter().filter(|n| n.starts_with("a.json.bak-")).collect();
        assert_eq!(a_backups.len(), 5);
        assert_eq!(a_backups[0], &backup_name("a.json", at(3)));
        assert_eq!(a_backups[4], &backup_name("a.json", at(7)));
        assert!(left.contains(&backup_name("b.json", at(0))));
        assert!(left.contains(&"a.json.bak".to_string()));
        assert!(left.contains(&"a.json".to_string()));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn quarantine_prunes_after_moving() {
        let d = temp_dir("quarantine-prune");
        for i in 0..BACKUPS_KEPT as i64 {
            std::fs::write(d.join(backup_name("a.json", at(i))), "x").unwrap();
        }
        std::fs::write(d.join("a.json"), "nuovo").unwrap();
        let newest = quarantine(&d, "a.json", at(100)).unwrap();
        let left = files(&d);
        assert_eq!(left.len(), BACKUPS_KEPT);
        assert!(left.contains(&newest));
        assert!(!left.contains(&backup_name("a.json", at(0))));
        let _ = std::fs::remove_dir_all(&d);
    }
}
