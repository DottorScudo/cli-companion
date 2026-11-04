//! Cartella di configurazione di OpenCode, come la riporta
//! `opencode debug paths` (righe `config     C:\Users\me\.config\opencode`).

use crate::agents::detection::PLUGIN_LIST_TIMEOUT;
use crate::platform::{wsl_to_windows_path, Target};
use std::path::PathBuf;

/// Etichetta della riga che contiene la cartella di configurazione.
const CONFIG_LABEL: &str = "config";

/// Path della riga `config` dell'output di `opencode debug paths`, cosi' come
/// lo vede l'agente (puo' contenere spazi).
fn parse_config_path(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (label, rest) = line.trim().split_once(char::is_whitespace)?;
        let path = rest.trim();
        (label == CONFIG_LABEL && !path.is_empty()).then(|| path.to_string())
    })
}

/// Il path dell'agente come lo legge Windows: invariato, o dalla distro WSL.
fn to_windows_path(target: &Target, agent_path: &str) -> Option<PathBuf> {
    match target {
        Target::Windows => Some(PathBuf::from(agent_path)),
        Target::Wsl { distro } => wsl_to_windows_path(distro, agent_path).map(PathBuf::from),
    }
}

/// Cartella di configurazione di OpenCode nel `target`, accessibile da Windows.
pub(super) fn config_dir(target: &Target, program: &str) -> Result<PathBuf, String> {
    let output = target
        .run_timeout(program, &["debug", "paths"], PLUGIN_LIST_TIMEOUT)
        .map_err(|e| format!("cartella di configurazione di OpenCode non leggibile: {e}"))?;
    let agent_path = parse_config_path(&output)
        .ok_or_else(|| "cartella di configurazione di OpenCode non trovata in 'debug paths'".to_string())?;
    to_windows_path(target, &agent_path)
        .ok_or_else(|| format!("cartella di configurazione di OpenCode non traducibile: {agent_path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_line_is_found_among_the_others() {
        let out = "home       C:\\Users\\jacob\n\
                   data       C:\\Users\\jacob\\.local\\share\\opencode\n\
                   config     C:\\Users\\jacob\\.config\\opencode\n\
                   state      C:\\Users\\jacob\\.local\\state\\opencode\n";
        assert_eq!(parse_config_path(out).as_deref(), Some(r"C:\Users\jacob\.config\opencode"));
    }

    #[test]
    fn config_path_keeps_inner_spaces_and_tolerates_crlf() {
        let out = "log  /x\r\nconfig   C:\\Users\\Mario Rossi\\.config\\opencode  \r\n";
        assert_eq!(parse_config_path(out).as_deref(), Some(r"C:\Users\Mario Rossi\.config\opencode"));
    }

    #[test]
    fn no_config_line_gives_none() {
        for out in ["", "config", "config   ", "configs /x", "data /x\nstate /y", "not paths at all"] {
            assert_eq!(parse_config_path(out), None, "{out:?}");
        }
    }

    #[test]
    fn the_path_is_readable_from_windows_for_each_target() {
        let wsl = Target::Wsl { distro: "Ubuntu".to_string() };
        assert_eq!(to_windows_path(&Target::Windows, r"C:\a\b"), Some(PathBuf::from(r"C:\a\b")));
        assert_eq!(
            to_windows_path(&wsl, "/home/me/.config/opencode"),
            Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\.config\opencode"))
        );
        assert_eq!(to_windows_path(&wsl, "/mnt/c/Users/me"), Some(PathBuf::from(r"C:\Users\me")));
        assert_eq!(to_windows_path(&wsl, "relative/path"), None);
    }
}
