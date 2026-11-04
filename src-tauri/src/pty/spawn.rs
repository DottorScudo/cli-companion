//! Avvio di una shell in un PTY: scelta della cartella, costruzione del
//! comando per target (Windows o WSL) e apertura della coppia master/slave.
//! La costruzione del comando e' pura e testabile; i controlli sul
//! filesystem e l'apertura del PTY sono passi separati.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};

use super::env::apply_env;
use crate::platform::{Target, WSL_EXE};

const DEFAULT_WINDOWS_SHELL: &str = "powershell.exe";
const MIN_ROWS: u16 = 2;
const MAX_ROWS: u16 = 300;
const MIN_COLS: u16 = 2;
const MAX_COLS: u16 = 500;

/// Hook iniettato in PowerShell via `-Command`: a ogni prompt riemette il
/// titolo console come sequenza `OSC 0` (`ESC ] 0 ; titolo BEL`).
///
/// Su Windows serve perche' ConPTY non inoltra nello stream VT i titoli
/// impostati via API Win32 (`$Host.UI.RawUI.WindowTitle = ...`, che e' come
/// gli agenti impostano il titolo): senza questo hook il tab non si aggiorna
/// mai. Se il titolo e' ancora quello di default, usa la cartella corrente.
/// Il `prompt` preesistente (profilo utente, oh-my-posh, ...) e' preservato.
pub(super) const POWERSHELL_TITLE_HOOK: &str = r#"$__mcInitTitle = ''; try { $__mcInitTitle = $Host.UI.RawUI.WindowTitle } catch { }; $__mcPrevPrompt = $null; try { $__mcPrevPrompt = (Get-Command prompt -CommandType Function -ErrorAction Stop).ScriptBlock } catch { }; function global:prompt { $__mcTitle = ''; try { $__mcTitle = $Host.UI.RawUI.WindowTitle; if ([string]::IsNullOrWhiteSpace($__mcTitle) -or $__mcTitle -eq $__mcInitTitle) { $__mcTitle = Split-Path -Leaf -Path (Get-Location).Path }; if ([string]::IsNullOrWhiteSpace($__mcTitle)) { $__mcTitle = 'PS' } } catch { $__mcTitle = 'PS' }; $__mcText = ''; try { if ($__mcPrevPrompt) { $__mcText = (@(& $__mcPrevPrompt) -join '') } else { $__mcText = "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) " } } catch { $__mcText = 'PS> ' }; "$([char]27)]0;$__mcTitle`a$__mcText" }"#;

pub(super) fn is_powershell(shell: &str) -> bool {
    let file = shell.rsplit(['/', '\\']).next().unwrap_or(shell).trim().to_lowercase();
    let stem = file.strip_suffix(".exe").unwrap_or(&file);
    matches!(stem, "powershell" | "pwsh")
}

pub(super) fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(MIN_ROWS, MAX_ROWS),
        cols: cols.clamp(MIN_COLS, MAX_COLS),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Cartella di partenza: quella richiesta (sessioni automatiche) oppure
/// quella del workspace.
pub(super) fn start_dir(requested: Option<&str>, workspace_path: &str) -> String {
    requested.map(str::trim).filter(|d| !d.is_empty()).unwrap_or(workspace_path).to_string()
}

/// Verifica che `dir` esista nel target. Su WSL interroga la distro con
/// `wsl.exe` (secondi a VM spenta): va chiamata fuori dal main thread.
pub(super) fn check_start_dir(target: &Target, dir: &str) -> Result<(), String> {
    match target {
        Target::Wsl { distro } => crate::wsl::ensure_wsl_dir(distro, dir),
        Target::Windows => {
            let meta = std::fs::metadata(dir).map_err(|_| format!("cartella non trovata: {dir}"))?;
            if meta.is_dir() {
                Ok(())
            } else {
                Err(format!("non è una cartella: {dir}"))
            }
        }
    }
}

/// Comando della shell nel target, senza ambiente. `shell` vuota o assente
/// significa "predefinita": PowerShell su Windows, la shell della distro su WSL.
pub(super) fn build_command(target: &Target, dir: &str, shell: Option<&str>) -> CommandBuilder {
    let shell = shell.filter(|s| !s.is_empty());
    match target {
        Target::Wsl { distro } => wsl_command(distro, dir, shell),
        Target::Windows => windows_command(dir, shell.unwrap_or(DEFAULT_WINDOWS_SHELL)),
    }
}

fn wsl_command(distro: &str, dir: &str, shell: Option<&str>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(WSL_EXE);
    cmd.args(["-d", distro, "--cd", dir]);
    if let Some(shell) = shell {
        cmd.args(["-e", shell]);
    }
    cmd
}

fn windows_command(dir: &str, shell: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if is_powershell(shell) {
        cmd.args(["-NoLogo", "-NoExit", "-Command", POWERSHELL_TITLE_HOOK]);
    }
    cmd.cwd(dir);
    cmd
}

/// Comando completo di una sessione: shell del target piu' ambiente (con le
/// variabili `staged` dell'integrazione, applicate dopo le nostre).
pub(super) fn session_command(
    target: &Target,
    dir: &str,
    shell: Option<&str>,
    tab_id: &str,
    secret: &str,
    staged: &[(String, String)],
) -> CommandBuilder {
    let mut cmd = build_command(target, dir, shell);
    apply_env(&mut cmd, tab_id, secret, target.kind(), staged);
    cmd
}

/// PTY aperto con la shell gia' avviata.
pub(super) struct OpenedPty {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

/// Apre il PTY e avvia `cmd`. Se i flussi non si ottengono la shell appena
/// avviata viene terminata: nessun processo resta senza proprietario.
pub(super) fn open_pty(cmd: CommandBuilder, size: PtySize) -> Result<OpenedPty, String> {
    let pair = native_pty_system().openpty(size).map_err(|e| format!("pty: {e}"))?;
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn: {e}"))?;
    drop(pair.slave);
    let streams = pair.master.try_clone_reader().map_err(|e| format!("pty reader: {e}")).and_then(|reader| {
        pair.master.take_writer().map(|writer| (reader, writer)).map_err(|e| format!("pty writer: {e}"))
    });
    match streams {
        Ok((reader, writer)) => Ok(OpenedPty { master: pair.master, child, reader, writer }),
        Err(e) => {
            let _ = child.kill();
            Err(e)
        }
    }
}
