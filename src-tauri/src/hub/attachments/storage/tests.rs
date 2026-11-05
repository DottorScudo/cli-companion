use super::*;
use crate::hub::attachments::Garbage;
use std::time::Duration;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

/// Radice degli allegati di prova, vuota.
fn temp_root(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cc-hub-attachments-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn bytes_are_staged_with_a_clean_name_and_a_detected_type() {
    let root = temp_root("bytes");
    let staged_png = stage_bytes(&root, "att-1-1", "..\\..\\incollata.png", PNG).unwrap();
    assert_eq!(staged_png.name, "incollata.png");
    assert_eq!((staged_png.mime.as_str(), staged_png.size), ("image/png", PNG.len() as u64));
    assert!(root.join("staging").join("att-1-1").join("incollata.png").is_file());
    assert_eq!(staged(&root, "att-1-1").unwrap(), staged_png);

    let text = stage_bytes(&root, "att-1-2", "CON.txt", b"ciao").unwrap();
    assert_eq!((text.name.as_str(), text.mime.as_str()), ("_CON.txt", "text/plain"));
    // Nessun file fuori dallo staging, qualunque sia il nome.
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn staging_rejects_bad_ids_names_and_oversized_files() {
    let root = temp_root("reject");
    assert!(stage_bytes(&root, "../x", "a.txt", b"x").is_err());
    assert!(stage_bytes(&root, "att-1-1", " . ", b"x").is_err());
    let big = vec![0u8; ATTACHMENT_SIZE_MAX as usize + 1];
    assert!(stage_bytes(&root, "att-1-2", "big.bin", &big).unwrap_err().contains("10 MB"));
    // Anche un lettore che va oltre il limite durante la copia.
    assert!(stage(&root, "att-1-3", "grows.bin", &big[..]).unwrap_err().contains("10 MB"));
    assert!(!root.join("staging").join("att-1-3").exists());
    assert!(staged(&root, "att-1-2").is_err() && staged(&root, "att-9-9").is_err());
}

#[test]
fn files_are_staged_from_their_path() {
    let root = temp_root("file");
    let source_dir = temp_root("file-source");
    fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("rapporto.pdf");
    fs::write(&source, b"%PDF-1.7 ...").unwrap();
    let staged_pdf = stage_file(&root, "att-1-1", &source).unwrap();
    assert_eq!((staged_pdf.name.as_str(), staged_pdf.mime.as_str()), ("rapporto.pdf", "application/pdf"));
    assert!(source.is_file(), "la sorgente resta");

    assert!(stage_file(&root, "att-1-2", &source_dir).unwrap_err().contains("non è un file"));
    assert!(stage_file(&root, "att-1-3", &source_dir.join("manca.txt")).is_err());
    let big = source_dir.join("big.bin");
    fs::write(&big, vec![0u8; ATTACHMENT_SIZE_MAX as usize + 1]).unwrap();
    assert!(stage_file(&root, "att-1-4", &big).unwrap_err().contains("10 MB"));
    assert!(!root.join("staging").join("att-1-4").exists());
    let _ = fs::remove_dir_all(source_dir);
}

#[test]
fn claiming_moves_staged_files_into_the_thread_and_release_brings_them_back() {
    let root = temp_root("claim");
    stage_bytes(&root, "att-1-1", "a.txt", b"a").unwrap();
    stage_bytes(&root, "att-1-2", "b.png", PNG).unwrap();
    let claimed = Claimed::take(&root, &ws(), "thr-1-1", &ids(&["att-1-1", "att-1-2"])).unwrap();
    assert_eq!(claimed.attachments().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["a.txt", "b.png"]);
    let stored = layout::stored_file(&root, &ws(), "thr-1-1", &claimed.attachments()[1]).unwrap();
    assert_eq!(read(&stored).unwrap(), PNG);
    assert!(staged(&root, "att-1-1").is_err());

    claimed.release();
    assert!(staged(&root, "att-1-1").is_ok() && staged(&root, "att-1-2").is_ok());
    assert!(!stored.exists());
}

#[test]
fn a_missing_attachment_releases_the_ones_already_claimed() {
    let root = temp_root("claim-missing");
    stage_bytes(&root, "att-1-1", "a.txt", b"a").unwrap();
    let err = Claimed::take(&root, &ws(), "thr-1-1", &ids(&["att-1-1", "att-1-9"])).unwrap_err();
    assert_eq!(err, NOT_STAGED);
    assert!(staged(&root, "att-1-1").is_ok());
}

#[test]
fn a_staged_attachment_can_be_claimed_only_once() {
    let root = temp_root("claim-once");
    stage_bytes(&root, "att-1-1", "a.txt", b"a").unwrap();
    let first = Claimed::take(&root, &ws(), "thr-1-1", &ids(&["att-1-1"])).unwrap();
    assert!(Claimed::take(&root, &ws(), "thr-1-2", &ids(&["att-1-1"])).is_err());
    assert_eq!(first.attachments().len(), 1);
}

#[test]
fn claiming_nothing_touches_no_folder() {
    let root = temp_root("claim-none");
    assert!(Claimed::take(&root, &ws(), "thr-1-1", &[]).unwrap().attachments().is_empty());
    assert!(!root.exists());
}

#[test]
fn discard_deletes_threads_and_single_attachments() {
    let root = temp_root("discard");
    for (thread, id) in [("thr-1", "att-1-1"), ("thr-1", "att-1-2"), ("thr-2", "att-1-3")] {
        stage_bytes(&root, id, "f.txt", b"x").unwrap();
        Claimed::take(&root, &ws(), thread, &ids(&[id])).unwrap();
    }
    let garbage = Garbage {
        threads: vec!["thr-2".to_string(), "thr-gone".to_string(), "../x".to_string()],
        attachments: vec![("thr-1".to_string(), "att-1-1".to_string())],
    };
    discard(&root, &ws(), &garbage);
    let thread_dir = |t: &str| layout::thread_dir(&root, &ws(), t).unwrap();
    assert!(!thread_dir("thr-2").exists());
    assert!(!thread_dir("thr-1").join("att-1-1").exists() && thread_dir("thr-1").join("att-1-2").is_dir());

    forget_channel(&root, &ws());
    assert!(!layout::channel_dir(&root, &ws()).exists());
    forget_channel(&root, &Channel::General);
}

#[test]
fn stale_entries_are_the_ones_modified_before_the_cutoff() {
    let cutoff = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let entries = vec![
        (PathBuf::from("old"), cutoff - Duration::from_secs(1)),
        (PathBuf::from("exact"), cutoff),
        (PathBuf::from("new"), cutoff + Duration::from_secs(1)),
    ];
    assert_eq!(stale(entries, cutoff), vec![PathBuf::from("old")]);
}

#[test]
fn purging_the_staging_removes_only_stale_uploads() {
    let root = temp_root("purge");
    purge_staging(&root, SystemTime::now());
    assert!(!root.exists(), "senza staging non crea nulla");
    stage_bytes(&root, "att-1-1", "a.txt", b"a").unwrap();
    purge_staging(&root, SystemTime::now() - Duration::from_secs(3600));
    assert!(staged(&root, "att-1-1").is_ok());
    purge_staging(&root, SystemTime::now() + Duration::from_secs(1));
    assert!(staged(&root, "att-1-1").is_err());
}

#[test]
fn reading_a_missing_attachment_is_a_clear_error() {
    let root = temp_root("read");
    assert_eq!(read(&root.join("nope")).unwrap_err(), NOT_FOUND);
}

#[test]
fn unused_staged_attachments_are_discarded_and_missing_ones_ignored() {
    let root = temp_root("discard-staged");
    stage_bytes(&root, "att-1-1", "a.txt", b"a").unwrap();
    stage_bytes(&root, "att-1-2", "b.txt", b"b").unwrap();
    discard_staged(&root, &ids(&["att-1-1", "att-9-9", "../x"]));
    assert!(staged(&root, "att-1-1").is_err());
    assert!(staged(&root, "att-1-2").is_ok());
    fs::remove_dir_all(&root).unwrap();
}
