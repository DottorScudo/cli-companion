//! Variabili d'ambiente di una sessione PTY: capacita' colore, identita'
//! della sessione per gli hook degli agenti e `WSLENV` per attraversare WSL.
//! `env_vars` e' pura (token e `WSLENV` ereditato entrano come argomenti);
//! `apply_env` e' l'unico punto che legge il processo.

use portable_pty::CommandBuilder;

use crate::ipc::{PORT_ENV, SESSION_ENV, TOKEN_ENV};
use crate::platform::TargetKind;

const WSLENV_VAR: &str = "WSLENV";

/// Hint di capacità colore per i figli del PTY (CLI Node/chalk, Python,
/// Rust termcolor...): senza TERM/COLORTERM molti tool disabilitano i colori
/// perché l'environment ereditato dall'app GUI non ne dichiara il supporto.
/// xterm.js gestisce 256 colori + truecolor, quindi li dichiariamo entrambi.
/// Niente FORCE_COLOR: forzerebbe i colori anche contro NO_COLOR/--no-color.
const COLOR_ENV: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

/// Aggiunge le `vars` a un valore di WSLENV (`A/p:B`), separato da `:`, senza
/// duplicarle se ci sono gia' (con o senza flag). Il flag `/u` le fa passare
/// da Windows alla distro; i processi Windows lanciati da li' (l'exe degli
/// hook, via interop) le ricevono ancora dall'ambiente d'origine.
pub(super) fn merge_wslenv(existing: Option<&str>, vars: &[&str]) -> String {
    let mut entries: Vec<String> =
        existing.unwrap_or("").trim_matches(':').split(':').filter(|e| !e.is_empty()).map(str::to_string).collect();
    for var in vars {
        if !entries.iter().any(|e| e.split('/').next() == Some(*var)) {
            entries.push(format!("{var}/u"));
        }
    }
    entries.join(":")
}

/// Tutte le variabili da impostare al figlio: colori, `CLI_COMPANION_SESSION`
/// (id del tab), `CLI_COMPANION_TOKEN` e `CLI_COMPANION_PORT` (se il listener
/// e' in ascolto) per gli hook degli agenti e il server MCP e, su WSL, `WSLENV`
/// che le fa attraversare il confine.
pub(super) fn env_vars(
    tab_id: &str,
    token: &str,
    port: Option<u16>,
    kind: TargetKind,
    parent_wslenv: Option<&str>,
) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = COLOR_ENV.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    vars.push((SESSION_ENV.to_string(), tab_id.to_string()));
    vars.push((TOKEN_ENV.to_string(), token.to_string()));
    let mut forwarded = vec![SESSION_ENV, TOKEN_ENV];
    if let Some(port) = port {
        vars.push((PORT_ENV.to_string(), port.to_string()));
        forwarded.push(PORT_ENV);
    }
    if kind == TargetKind::Wsl {
        vars.push((WSLENV_VAR.to_string(), merge_wslenv(parent_wslenv, &forwarded)));
    }
    vars
}

/// Imposta sul comando l'ambiente della sessione `tab_id`.
pub(super) fn apply_env(cmd: &mut CommandBuilder, tab_id: &str, kind: TargetKind) {
    let parent_wslenv = std::env::var(WSLENV_VAR).ok();
    let port = crate::ipc::listening_port();
    for (key, value) in env_vars(tab_id, crate::automation::token(), port, kind, parent_wslenv.as_deref()) {
        cmd.env(key, value);
    }
    // Senza listener non si eredita la porta di un'altra istanza (app avviata da un suo terminale).
    if port.is_none() {
        cmd.env_remove(PORT_ENV);
    }
}
