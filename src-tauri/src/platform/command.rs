//! Costruzione dei comandi per target: l'unico punto che conosce `cmd /c`,
//! `wsl.exe` e il flag di finestra nascosta.

use super::exec::output_with_timeout;
use super::Target;
use std::process::Command;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Eseguibile che lancia i comandi dentro una distro WSL.
pub(crate) const WSL_EXE: &str = "wsl.exe";
/// Timeout predefinito di `Target::run`.
const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(120);
/// Impedisce a git di chiedere credenziali su un terminale che non c'e'.
const GIT_NO_PROMPT_ENV: (&str, &str) = ("GIT_TERMINAL_PROMPT", "0");

/// Niente finestra console per i processi figli: la build release e' una app
/// GUI (windows_subsystem) e un figlio console aprirebbe un terminale
/// lampeggiante. No-op fuori da Windows.
pub(crate) fn hide_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// `wsl.exe` senza finestra: unico costruttore dei comandi WSL.
pub(crate) fn wsl_exe() -> Command {
    let mut cmd = Command::new(WSL_EXE);
    hide_window(&mut cmd);
    cmd
}

/// Script `bash -lc` che esegue `abs_path` con gli argomenti posizionali.
/// Il path e' tra apici singoli (quelli interni sono escapati).
fn wsl_login_script(abs_path: &str) -> String {
    format!("'{}' \"$@\"", abs_path.replace('\'', "'\\''"))
}

/// Comando `argv` nella distro, con `dir` come cartella corrente. `-e`
/// (exec) e' indispensabile: con `--` wsl.exe passa la riga alla shell di
/// default della distro, che rompe spazi, `;` e apici negli argomenti.
pub(crate) fn wsl_run_command(distro: &str, dir: &str, argv: &[&str]) -> Command {
    let mut cmd = wsl_exe();
    cmd.args(["-d", distro, "--cd", dir, "-e"]).args(argv).stdin(std::process::Stdio::null());
    cmd
}

impl Target {
    /// Comando che esegue `program` nel target. Su Windows passa da
    /// `cmd /c`: gli shim npm (.cmd) non si risolvono con uno spawn diretto
    /// (CreateProcess cerca solo .exe), `cmd` applica PATHEXT. Su WSL il
    /// programma e' di norma un path assoluto (`resolve_wsl_command`), eseguito
    /// tramite `bash -lc` (vedi `wsl_login_script`).
    pub fn command(&self, program: &str) -> Command {
        match self {
            #[cfg(windows)]
            Target::Windows => {
                let mut cmd = Command::new("cmd");
                cmd.args(["/c", program]);
                hide_window(&mut cmd);
                cmd
            }
            #[cfg(not(windows))]
            Target::Windows => Command::new(program),
            Target::Wsl { distro } => {
                let mut cmd = wsl_exe();
                // `-e` (exec diretto): con `--` wsl.exe riunisce gli argomenti e li
                // rilegge la shell di default della distro, perdendo virgolette
                // e argomenti posizionali.
                cmd.args(["-d", distro, "-e"]);
                if program.starts_with('/') {
                    // Path assoluto (risolto con una login shell): va eseguito
                    // dalla stessa login shell, altrimenti un eseguibile con
                    // `#!/usr/bin/env node` (claude installato con nvm) non
                    // trova `node` nel PATH non-login. Gli argomenti seguono
                    // `_` e arrivano come "$@": nessuna interpolazione.
                    cmd.args(["bash", "-lc", &wsl_login_script(program), "_"]);
                } else {
                    cmd.arg(program);
                }
                cmd
            }
        }
    }

    /// `git <args>` in `dir` (path nel formato del target), senza prompt di
    /// credenziali.
    pub fn git_command(&self, dir: &str, args: &[&str]) -> Command {
        match self {
            Target::Windows => {
                let mut cmd = Command::new("git");
                cmd.args(args).current_dir(dir).env(GIT_NO_PROMPT_ENV.0, GIT_NO_PROMPT_ENV.1);
                hide_window(&mut cmd);
                cmd
            }
            Target::Wsl { distro } => {
                let mut argv = vec!["git"];
                argv.extend_from_slice(args);
                wsl_run_command(distro, dir, &argv)
            }
        }
    }

    /// Esegue `program args...`; ritorna lo stdout, o in errore lo stderr
    /// (o lo stdout, o l'exit code).
    pub fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        self.run_timeout(program, args, DEFAULT_RUN_TIMEOUT)
    }

    /// Come `run` con un timeout esplicito (il figlio viene ucciso allo scadere).
    pub fn run_timeout(&self, program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
        run_command(self.command(program), args, program, timeout)
    }
}

fn run_command(mut cmd: Command, args: &[&str], label: &str, timeout: Duration) -> Result<String, String> {
    let out = output_with_timeout(cmd.args(args), timeout).map_err(|e| format!("{label} non avviabile: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if err.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { err };
        return Err(if msg.is_empty() { format!("{label} uscito con codice {:?}", out.status.code()) } else { msg });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(c: &Command) -> Vec<String> {
        c.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    fn wsl(distro: &str) -> Target {
        Target::Wsl { distro: distro.to_string() }
    }

    #[test]
    fn command_wraps_windows_cmd_shim_and_wsl() {
        let c = Target::Windows.command("claude");
        #[cfg(windows)]
        {
            assert_eq!(c.get_program(), "cmd");
            assert_eq!(args(&c), ["/c", "claude"]);
        }
        #[cfg(not(windows))]
        assert_eq!(c.get_program(), "claude");
        let w = wsl("Ubuntu-24.04").command("/home/u/.local/bin/claude");
        assert_eq!(w.get_program(), WSL_EXE);
        assert_eq!(args(&w), ["-d", "Ubuntu-24.04", "-e", "bash", "-lc", "'/home/u/.local/bin/claude' \"$@\"", "_"]);
        // Programma non assoluto (es. `bash` per la risoluzione): invariato.
        assert_eq!(args(&wsl("U").command("bash")), ["-d", "U", "-e", "bash"]);
    }

    #[test]
    fn wsl_login_script_quotes_path_and_passes_args_positionally() {
        assert_eq!(wsl_login_script("/a b/claude"), "'/a b/claude' \"$@\"");
        assert_eq!(wsl_login_script("/it's/claude"), "'/it'\\''s/claude' \"$@\"");
        // Gli argomenti di run() vengono dopo `_` e non toccano lo script.
        let mut c = wsl("U").command("/x/claude");
        c.args(["plugin", "list", "--json"]);
        let a = args(&c);
        assert_eq!(&a[a.len() - 4..], ["_", "plugin", "list", "--json"]);
    }

    #[test]
    fn run_and_git_use_exec_so_argv_skips_the_distro_shell() {
        let c = wsl_run_command("Ubuntu", "/p q", &["sh", "-c", "echo $1", "sh", "a b;c"]);
        assert_eq!(args(&c), ["-d", "Ubuntu", "--cd", "/p q", "-e", "sh", "-c", "echo $1", "sh", "a b;c"]);
        let g = wsl("Ubuntu").git_command("/p", &["worktree", "add", "/p/.worktrees/a b"]);
        assert_eq!(g.get_program(), WSL_EXE);
        assert_eq!(args(&g), ["-d", "Ubuntu", "--cd", "/p", "-e", "git", "worktree", "add", "/p/.worktrees/a b"]);
    }

    #[test]
    fn windows_git_runs_in_dir_without_credential_prompts() {
        let g = Target::Windows.git_command("C:\\p", &["status"]);
        assert_eq!(g.get_program(), "git");
        assert_eq!(args(&g), ["status"]);
        assert_eq!(g.get_current_dir(), Some(std::path::Path::new("C:\\p")));
        let env: Vec<_> = g.get_envs().collect();
        assert!(env.iter().any(|(k, v)| *k == "GIT_TERMINAL_PROMPT" && *v == Some(std::ffi::OsStr::new("0"))));
    }

    #[test]
    fn run_command_reports_spawn_failures_with_the_label() {
        let err = run_command(Command::new("mc-no-such-program"), &[], "fake", Duration::from_secs(5)).unwrap_err();
        assert!(err.starts_with("fake non avviabile"), "{err}");
    }

    /// Regressione "agente non trovato" su Windows: `claude`/`npm` si risolvono
    /// a shim .cmd e CreateProcess cerca solo .exe per nome, quindi lo spawn
    /// diretto fallisce mentre `cmd /c` (PATHEXT) lo trova. Prova con uno
    /// shim finto in temp (niente dipendenza da un agente installato).
    #[test]
    #[cfg(windows)]
    fn cmd_shim_found_only_through_cmd_c() {
        let dir = std::env::temp_dir().join(format!("mc-cmd-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mc-fake-shim.cmd"), "@echo fake-shim-ok\r\n").unwrap();

        // Solo lo shim in testa al PATH (ripristinato subito dopo gli spawn).
        let prev_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![dir.clone()];
        paths.extend(std::env::split_paths(&prev_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());

        // Spawn diretto per nome: non risolve. Via Target::run (cmd /c): si'.
        let direct = run_command(Command::new("mc-fake-shim"), &[], "shim", DEFAULT_RUN_TIMEOUT);
        let via_cmd = Target::Windows.run("mc-fake-shim", &[]);

        std::env::set_var("PATH", &prev_path);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(direct.is_err(), "lo spawn diretto non deve risolvere gli shim .cmd");
        assert!(via_cmd.expect("cmd /c deve risolvere gli shim .cmd").contains("fake-shim-ok"));
    }
}
