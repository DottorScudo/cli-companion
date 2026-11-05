//! Cio' che e' davvero WSL: distro installate, home, verifica delle cartelle.
//! Il lancio dei processi e le conversioni di path stanno in `platform`.

use crate::platform::{output_with_timeout, windows_to_wsl_path, wsl_exe};
use crate::ttl_cache::TtlCache;
use std::process::Output;
use std::sync::OnceLock;
use std::time::Duration;

mod home;

pub use home::linux_home;

/// wsl.exe scrive in UTF-16LE con BOM: decodifica tollerante (UTF-16LE/BE o UTF-8).
fn decode_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
        String::from_utf16_lossy(&units)
    } else if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().map(|&c| u16::from_be_bytes(c)).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Validita' della cache delle distro installate.
const DISTROS_CACHE_TTL: Duration = Duration::from_secs(60);
/// `wsl -l -q` non deve poter bloccare l'app se WSL e' appeso.
const LIST_DISTROS_TIMEOUT: Duration = Duration::from_secs(10);

fn distros_cache() -> &'static TtlCache<(), Vec<String>> {
    static C: OnceLock<TtlCache<(), Vec<String>>> = OnceLock::new();
    C.get_or_init(|| TtlCache::new(DISTROS_CACHE_TTL))
}

/// Svuota la cache delle distro (dopo install/remove).
pub(crate) fn invalidate_distros_cache() {
    distros_cache().clear();
}

/// Distro installate, con cache di breve durata (solo gli esiti positivi).
pub fn installed_distros() -> Result<Vec<String>, String> {
    if let Some(v) = distros_cache().get(&()) {
        return Ok(v);
    }
    let v = installed_distros_uncached()?;
    distros_cache().put((), v.clone());
    Ok(v)
}

fn installed_distros_uncached() -> Result<Vec<String>, String> {
    let output = output_with_timeout(wsl_exe().args(["-l", "-q"]), LIST_DISTROS_TIMEOUT)
        .map_err(|e| format!("wsl.exe non disponibile: {e}"))?;
    let text = decode_output(&output.stdout);
    let mut names: Vec<String> =
        text.lines().map(|l| l.replace('\0', "").trim().to_string()).filter(|l| !l.is_empty()).collect();
    // Difesa contro output localizzati o senza flag -q (vecchie build).
    names.retain(|n| {
        let lower = n.to_lowercase();
        !(lower.starts_with("windows subsystem") || lower.starts_with("per ") || lower.contains("installer"))
    });
    Ok(names)
}

/// Verificare una cartella puo' richiedere l'avvio della VM (secondi), ma
/// una WSL appesa non deve bloccare chi chiama.
const DIR_CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// Exit code di `test -d` quando la cartella non esiste.
const TEST_FALSE_EXIT_CODE: i32 = 1;

/// `Ok(true/false)` = la cartella c'e' / non c'e'. `Err` = WSL non ha
/// risposto (timeout, distro assente o ferma, `wsl.exe` non avviabile): non
/// si sa se la cartella esista, e il messaggio lo dice.
pub fn wsl_dir_exists(distro: &str, path: &str) -> Result<bool, String> {
    let mut cmd = wsl_exe();
    cmd.args(["-d", distro, "-e", "test", "-d", path]);
    dir_check_outcome(distro, output_with_timeout(&mut cmd, DIR_CHECK_TIMEOUT))
}

fn dir_check_outcome(distro: &str, output: std::io::Result<Output>) -> Result<bool, String> {
    let unreachable = |reason: String| format!("WSL non risponde nella distro {distro}: {reason}");
    let output = output.map_err(|e| unreachable(e.to_string()))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(TEST_FALSE_EXIT_CODE) => Ok(false),
        code => {
            let stderr = decode_output(&output.stderr);
            let detail = stderr.trim();
            Err(unreachable(if detail.is_empty() { format!("exit code {code:?}") } else { detail.to_string() }))
        }
    }
}

/// Come `wsl_dir_exists`, ma con l'esito pronto per l'utente: `Err` sia se
/// la cartella manca sia se WSL non risponde (con messaggi diversi).
pub fn ensure_wsl_dir(distro: &str, path: &str) -> Result<(), String> {
    if wsl_dir_exists(distro, path)? {
        Ok(())
    } else {
        Err(format!("cartella non trovata in {distro}: {path}"))
    }
}

#[tauri::command]
pub async fn list_wsl_distros() -> Result<Vec<String>, String> {
    crate::blocking(installed_distros).await
}

#[tauri::command]
pub async fn check_wsl_path(distro: String, path: String) -> Result<bool, String> {
    if distro.trim().is_empty() || path.trim().is_empty() {
        return Ok(false);
    }
    let path = path.trim();
    let linux_path = if path.starts_with('/') {
        path.to_string()
    } else if let Some(converted) = windows_to_wsl_path(path) {
        converted
    } else {
        return Ok(false);
    };
    crate::blocking(move || wsl_dir_exists(distro.trim(), &linux_path)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(text: &str) -> Vec<u8> {
        [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect()
    }

    #[cfg(windows)]
    fn output_with_code(code: u32, stderr: Vec<u8>) -> std::io::Result<Output> {
        use std::os::windows::process::ExitStatusExt;
        Ok(Output { status: std::process::ExitStatus::from_raw(code), stdout: Vec::new(), stderr })
    }

    #[cfg(windows)]
    #[test]
    fn dir_check_tells_a_missing_folder_from_an_unresponsive_wsl() {
        assert_eq!(dir_check_outcome("U", output_with_code(0, Vec::new())), Ok(true));
        assert_eq!(dir_check_outcome("U", output_with_code(1, Vec::new())), Ok(false));
        // Distro inesistente: wsl.exe esce con -1 e scrive il motivo in UTF-16LE.
        let err = dir_check_outcome("U", output_with_code(u32::MAX, utf16le("no distro"))).unwrap_err();
        assert_eq!(err, "WSL non risponde nella distro U: no distro");
        let err = dir_check_outcome("U", output_with_code(2, Vec::new())).unwrap_err();
        assert!(err.contains("WSL non risponde") && err.contains("exit code"), "{err}");
    }

    #[test]
    fn dir_check_reports_timeouts_and_spawn_failures_as_unresponsive() {
        let timed_out = Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "nessuna risposta entro 20s"));
        assert_eq!(
            dir_check_outcome("U", timed_out).unwrap_err(),
            "WSL non risponde nella distro U: nessuna risposta entro 20s"
        );
        let missing = Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(dir_check_outcome("U", missing).unwrap_err().starts_with("WSL non risponde"));
    }

    #[test]
    fn decode_output_handles_utf16_and_utf8() {
        assert_eq!(decode_output(&utf16le("Ubuntu")), "Ubuntu");
        let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("Ubuntu".encode_utf16().flat_map(u16::to_be_bytes)).collect();
        assert_eq!(decode_output(&be), "Ubuntu");
        assert_eq!(decode_output(b"Ubuntu"), "Ubuntu");
    }
}
