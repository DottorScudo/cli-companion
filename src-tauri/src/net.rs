//! Client HTTP condiviso: un sottoprocesso `curl`, senza dipendenze di rete in
//! piu' nel binario. Lo usano l'updater (release su GitHub) e le integrazioni
//! degli agenti (es. limiti di utilizzo).
//!
//! Scelta: si resta su `curl` (preinstallato su Windows 10+ e nelle distro) invece
//! di aggiungere un crate HTTP (`reqwest`/`ureq` portano TLS e decine di
//! dipendenze per due richieste GET sporadiche).

use crate::platform::hide_window;
use std::io::Write;
use std::process::{Command, Stdio};

/// Variabile d'ambiente che sostituisce il binario `curl` (test, mock).
const CURL_OVERRIDE_ENV: &str = "CLI_COMPANION_UPDATER_CURL";

/// Valore di una variabile d'ambiente, trim; `None` se assente o vuota.
pub(crate) fn env_override(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn curl_bin() -> String {
    env_override(CURL_OVERRIDE_ENV).unwrap_or_else(|| if cfg!(windows) { "curl.exe" } else { "curl" }.to_string())
}

pub(crate) fn run_curl(args: &[String]) -> Result<Vec<u8>, String> {
    run_curl_with(args, None)
}

/// Come `run_curl` ma scrive `stdin` sullo stdin di curl: serve per passare
/// header segreti con `-H @-` senza metterli nella riga di comando.
pub(crate) fn run_curl_stdin(args: &[String], stdin: &[u8]) -> Result<Vec<u8>, String> {
    run_curl_with(args, Some(stdin))
}

fn run_curl_with(args: &[String], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let bin = curl_bin();
    let spawn_err = |e: std::io::Error| {
        format!(
            "Impossibile eseguire '{bin}' (curl non trovato nel PATH?): {e}. Su Windows 10+ curl.exe è preinstallato."
        )
    };
    let mut cmd = Command::new(&bin);
    hide_window(&mut cmd).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().map_err(spawn_err)?;
    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // Errore ignorato: se curl chiude prima, l'esito si vede dall'exit code.
        let _ = pipe.write_all(data);
    }
    let out = child.wait_with_output().map_err(spawn_err)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("curl fallito (exit {}): {}", out.status, err.trim()));
    }
    Ok(out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_trims_and_ignores_empty_values() {
        const VAR: &str = "CLI_COMPANION_NET_TEST_OVERRIDE";
        assert_eq!(env_override(VAR), None);
        std::env::set_var(VAR, "   ");
        assert_eq!(env_override(VAR), None);
        std::env::set_var(VAR, "  value ");
        assert_eq!(env_override(VAR).as_deref(), Some("value"));
        std::env::remove_var(VAR);
    }

    #[test]
    fn a_missing_binary_is_reported_with_its_name() {
        let args = vec!["--version".to_string()];
        let err = {
            std::env::set_var(CURL_OVERRIDE_ENV, "definitely-not-a-curl-binary");
            let result = run_curl(&args);
            std::env::remove_var(CURL_OVERRIDE_ENV);
            result.unwrap_err()
        };
        assert!(err.contains("definitely-not-a-curl-binary"), "{err}");
    }
}
