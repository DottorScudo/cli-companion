//! Scrittura atomica dei file: temporaneo sincronizzato + rename.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Tentativi di rename su Windows: antivirus e indicizzatore possono bloccare
/// il file per qualche millisecondo.
const RENAME_ATTEMPTS: u32 = 5;
/// Pausa base fra i tentativi; il ritardo cresce con il numero del tentativo.
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(20);

/// Esegue `op` fino a `attempts` volte (almeno una), dormendo `delay` per il
/// numero del tentativo fra un fallimento e il successivo. Ritorna l'ultimo
/// errore se tutti i tentativi falliscono.
fn retry<T>(attempts: u32, delay: Duration, mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 1;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt >= attempts => return Err(e),
            Err(_) => {
                std::thread::sleep(delay * attempt);
                attempt += 1;
            }
        }
    }
}

fn write_synced(path: &Path, data: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.sync_all()
}

/// Scrittura atomica: file temporaneo + `sync_all` + rename, cosi' un crash a
/// meta' salvataggio non lascia un JSON troncato. Il temporaneo viene rimosso
/// se qualcosa fallisce. Il rename riprova solo su Windows.
pub(crate) fn write_atomic(path: &Path, data: &str) -> io::Result<()> {
    // Nome unico per scrittura: due salvataggi concorrenti dello stesso file
    // (debounce + chiusura) non devono pestarsi il temporaneo.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{n}.tmp"));
    let attempts = if cfg!(windows) { RENAME_ATTEMPTS } else { 1 };
    let result =
        write_synced(&tmp, data).and_then(|()| retry(attempts, RENAME_RETRY_DELAY, || std::fs::rename(&tmp, path)));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-atomic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn fail() -> io::Error {
        io::Error::new(ErrorKind::PermissionDenied, "bloccato")
    }

    #[test]
    fn retry_succeeds_after_some_failures() {
        let mut calls = 0;
        let r = retry(5, Duration::ZERO, || {
            calls += 1;
            if calls < 3 {
                Err(fail())
            } else {
                Ok(calls)
            }
        });
        assert_eq!(r.unwrap(), 3);
        assert_eq!(calls, 3);
    }

    #[test]
    fn retry_gives_up_with_the_last_error() {
        let mut calls = 0;
        let r: io::Result<()> = retry(4, Duration::ZERO, || {
            calls += 1;
            Err(io::Error::other(format!("errore {calls}")))
        });
        assert_eq!(calls, 4);
        assert_eq!(r.unwrap_err().to_string(), "errore 4");
    }

    #[test]
    fn retry_always_tries_once() {
        let mut calls = 0;
        let r: io::Result<()> = retry(0, Duration::ZERO, || {
            calls += 1;
            Err(fail())
        });
        assert!(r.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn overwrites_and_leaves_no_temp_file() {
        let d = temp_dir("overwrite");
        let path = d.join("a.json");
        write_atomic(&path, "uno").unwrap();
        write_atomic(&path, "due").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "due");
        let names: Vec<String> =
            std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["a.json".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failure_removes_the_temp_file() {
        let d = temp_dir("fail");
        // Il destinatario e' una cartella: il rename fallisce.
        let target = d.join("a.json");
        std::fs::create_dir_all(&target).unwrap();
        assert!(write_atomic(&target, "x").is_err());
        let leftovers = std::fs::read_dir(&d).unwrap().filter(|e| e.as_ref().unwrap().path().is_file()).count();
        assert_eq!(leftovers, 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_writers_leave_a_valid_file() {
        const WRITERS: usize = 8;
        const ROUNDS: usize = 10;
        let d = temp_dir("threads");
        let path = d.join("a.json");
        let handles: Vec<_> = (0..WRITERS)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    // Sotto contesa estrema Windows puo' rifiutare un rename anche dopo i tentativi:
                    // l'invariante e' che il file non sia mai troncato e non resti nulla in giro.
                    (0..ROUNDS)
                        .filter(|j| write_atomic(&path, &format!(r#"{{"writer":{i},"round":{j}}}"#)).is_ok())
                        .count()
                })
            })
            .collect();
        let succeeded: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert!(succeeded > 0);
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(v["writer"].is_u64() && v["round"].is_u64());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "nessun temporaneo rimasto");
        let _ = std::fs::remove_dir_all(&d);
    }
}
