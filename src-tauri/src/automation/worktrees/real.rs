//! Lato reale dell'`Host` per git e cartelle del workspace: esegue git, rileva
//! i repository, aggiorna il file di esclusione. Ogni processo passa da
//! `process::run_command` (o `run_wsl`): output limitato e, a timeout,
//! terminazione dell'intero albero di processi (in WSL anche i processi Linux).

use super::{GitOut, FETCH_TIMEOUT, GIT_TIMEOUT};
use crate::automation::process::{run_command, run_wsl, KillTree};
use crate::automation::{clip_chars, Workspace};
use crate::platform::Target;
use crate::validate::normalize_repo;

/// Limite dei repository elencati (il resto si ignora).
const MAX_REPOS: usize = 200;

/// Esegue `git <args>` in `dir`. `Err` solo se git non parte o scade.
pub(in crate::automation) fn run_git(ws: &Workspace, dir: &str, args: &[&str]) -> Result<GitOut, String> {
    let limit = if args.first() == Some(&"fetch") { FETCH_TIMEOUT } else { GIT_TIMEOUT };
    let out = match ws.target()? {
        Target::Wsl { distro } => {
            let mut argv = vec!["git"];
            argv.extend_from_slice(args);
            run_wsl(&distro, dir, &argv, limit)
        }
        target @ Target::Windows => run_command(target.git_command(dir, args), limit, None, &KillTree::Native),
    };
    out.map_err(|e| format!("git {}: {e}", args.join(" ")))
}

/// Elenco dei repository da un elenco di nomi (uno per riga): normalizzati,
/// senza doppioni, "." per primo.
pub(in crate::automation) fn parse_repo_list(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| normalize_repo(l).ok().map(|r| r.unwrap_or_else(|| ".".to_string())))
        .collect();
    v.sort_by(|a, b| (a != ".", a).cmp(&(b != ".", b)));
    v.dedup();
    v.truncate(MAX_REPOS);
    v
}

/// Repository del workspace: "." se la root stessa e' un repo, piu' le
/// sottocartelle dirette (profondita' 1, senza quelle nascoste) con un `.git`
/// (cartella o file). WSL: una sola chiamata `sh`.
pub(in crate::automation) fn detect_repos(ws: &Workspace) -> Result<Vec<String>, String> {
    if let Target::Wsl { distro } = ws.target()? {
        let script = "for d in */; do [ -e \"${d}.git\" ] && printf '%s\\n' \"${d%/}\"; done; [ -e .git ] && printf '.\\n'; exit 0";
        let out = run_wsl(&distro, &ws.path, &["sh", "-c", script], GIT_TIMEOUT)?;
        if !out.ok {
            return Err(format!("repository detection failed: {}", clip_chars(out.stderr.trim(), 200)));
        }
        return Ok(parse_repo_list(&out.stdout));
    }
    let root = std::path::Path::new(&ws.path);
    let mut names = Vec::new();
    if root.join(".git").exists() {
        names.push(".".to_string());
    }
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') && entry.path().is_dir() && entry.path().join(".git").exists() {
            names.push(name);
        }
    }
    Ok(parse_repo_list(&names.join("\n")))
}

/// Aggiunge `line` al file di esclusione (se manca). WSL: uno script `sh`
/// con file e riga passati come argv (mai interpolati; `-e`, senza la shell
/// di default della distro di mezzo).
pub(in crate::automation) fn append_exclude_line(ws: &Workspace, file: &str, line: &str) -> Result<(), String> {
    if let Target::Wsl { distro } = ws.target()? {
        let script = "mkdir -p \"$(dirname \"$1\")\" && { grep -qxF -e \"$2\" \"$1\" 2>/dev/null || printf '%s\n' \"$2\" >> \"$1\"; }";
        let out = run_wsl(&distro, &ws.path, &["sh", "-c", script, "sh", file, line], GIT_TIMEOUT)?;
        return if out.ok {
            Ok(())
        } else {
            Err(format!("exclude not updated: {}", clip_chars(out.stderr.trim(), 200)))
        };
    }
    let path = std::path::Path::new(file);
    let current = std::fs::read_to_string(path).unwrap_or_default();
    if current.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(line);
    next.push('\n');
    std::fs::write(path, next).map_err(|e| e.to_string())
}
