//! Rilevamento dei target (Windows + distro WSL) e risoluzione dei programmi
//! di un agente al loro interno.

use crate::platform::{is_windows_drive_path, output_with_timeout, Target};
use crate::ttl_cache::TtlCache;
use crate::wsl::{installed_distros, invalidate_distros_cache};
use std::sync::OnceLock;
use std::time::Duration;

/// Timeout della risoluzione del path (shell login/interattiva) e di `--version`.
pub(crate) const DETECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Timeout di `plugin list` (l'agente puo' consultare i marketplace).
pub(crate) const PLUGIN_LIST_TIMEOUT: Duration = Duration::from_secs(20);
/// Validita' della cache dei path risolti.
const RESOLVE_CACHE_TTL: Duration = Duration::from_secs(60);

/// Primo token che inizia con una cifra (`2.1.211 (Claude Code)` -> `2.1.211`).
pub fn parse_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .map(|t| t.trim_matches(|c: char| c == ',' || c == ';').to_string())
}

/// Riga che separa il rumore della shell dall'elenco dei candidati.
const CANDIDATES_MARKER: &str = "__cli_companion_candidates__";

/// Filesystem dei drive Windows montati in WSL (WSL2: 9p, WSL1: drvfs).
const WINDOWS_FS_TYPES: &[&str] = &["9p", "v9fs", "drvfs"];

/// Script che stampa il marcatore e poi, in ordine di PATH, ogni eseguibile
/// `name` come `<tipo fs>\t<path>`.
fn candidates_script(name: &str) -> String {
    format!(
        "printf '%s\\n' {CANDIDATES_MARKER}; type -ap '{}' | while IFS= read -r p; do \
         printf '%s\\t%s\\n' \"$(stat -f -c %T -- \"$p\" 2>/dev/null)\" \"$p\"; done",
        name.replace('\'', "'\\''")
    )
}

/// Primo candidato Linux dall'output di `candidates_script`. Le shell
/// interattive/login stampano rumore (motd, nvm, avvisi) prima del
/// marcatore. Si scartano i programmi sui drive Windows: WSL accoda il PATH
/// di Windows, e lo shim npm `/mnt/c/.../npm/claude` avvierebbe il
/// `claude.exe` di Windows, con la sua config e i path tradotti male.
pub fn parse_command_candidates(output: &str) -> Option<String> {
    let (_, list) = output.rsplit_once(CANDIDATES_MARKER)?;
    list.lines()
        .filter_map(|l| {
            let l = l.trim_end_matches('\r');
            let (fs, path) = l.split_once('\t').unwrap_or(("", l));
            Some((fs.trim(), path.trim())).filter(|(_, p)| p.starts_with('/'))
        })
        .find(|(fs, path)| !WINDOWS_FS_TYPES.contains(fs) && !is_windows_drive_path(path))
        .map(|(_, path)| path.to_string())
}

/// Risolve il path assoluto di `name` in una distro, ignorando i programmi
/// di Windows visibili dal PATH. Il `wsl -d D -e name` diretto usa una
/// shell non-login e spesso non vede `~/.local/bin` o nvm: si chiede a una
/// login shell, con ripiego su quella interattiva.
pub fn resolve_wsl_command(distro: &str, name: &str) -> Option<String> {
    let script = candidates_script(name);
    for flag in ["-lc", "-ic"] {
        let mut cmd = Target::Wsl { distro: distro.to_string() }.command("bash");
        // Lo stdout basta (anche con exit != 0 per il rumore); lo stderr si ignora.
        if let Ok(out) = output_with_timeout(cmd.args([flag, &script]), DETECT_TIMEOUT) {
            if let Some(p) = parse_command_candidates(&String::from_utf8_lossy(&out.stdout)) {
                return Some(p);
            }
        }
    }
    None
}

/// `resolve_wsl_command` con cache di breve durata (anche gli esiti `None`).
pub fn resolve_wsl_command_cached(distro: &str, name: &str) -> Option<String> {
    let key = (distro.to_string(), name.to_string());
    if let Some(v) = resolve_cache().get(&key) {
        return v;
    }
    let v = resolve_wsl_command(distro, name);
    resolve_cache().put(key, v.clone());
    v
}

fn resolve_cache() -> &'static TtlCache<(String, String), Option<String>> {
    static C: OnceLock<TtlCache<(String, String), Option<String>>> = OnceLock::new();
    C.get_or_init(|| TtlCache::new(RESOLVE_CACHE_TTL))
}

/// Dimentica distro e path risolti: il rilevamento successivo e' fresco.
pub(crate) fn invalidate_detection_cache() {
    invalidate_distros_cache();
    resolve_cache().clear();
}

/// Windows + distro WSL installate (senza quelle interne di Docker, che non
/// hanno una shell utilizzabile).
pub(crate) fn list_targets() -> Vec<Target> {
    let mut targets = vec![Target::Windows];
    targets.extend(
        installed_distros()
            .unwrap_or_default()
            .into_iter()
            .filter(|d| !d.to_lowercase().starts_with("docker-desktop"))
            .map(|distro| Target::Wsl { distro }),
    );
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_first_numeric_token() {
        assert_eq!(parse_version("2.1.211 (Claude Code)\n").as_deref(), Some("2.1.211"));
        assert_eq!(parse_version("Claude Code 1.2.3, build").as_deref(), Some("1.2.3"));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("   \n"), None);
    }

    #[test]
    fn command_candidates_skip_noise_and_keep_path_order() {
        let p = |s: &str| parse_command_candidates(s);
        let m = CANDIDATES_MARKER;
        assert_eq!(
            p(&format!("{m}\next2/ext3\t/home/u/.local/bin/claude\n")).as_deref(),
            Some("/home/u/.local/bin/claude")
        );
        // Rumore prima del marcatore (anche righe che sembrano path).
        let noisy = format!(
            "bash: cannot set terminal process group (1): Inappropriate ioctl\r\n\
             Welcome!\r\n/usr/bin/other-noise\r\n{m}\r\next2/ext3\t/home/u/.nvm/bin/claude\r\n\
             ext2/ext3\t/usr/bin/claude\r\n\r\n"
        );
        assert_eq!(p(&noisy).as_deref(), Some("/home/u/.nvm/bin/claude"));
        // Path con spazi; tipo fs assente (stat non disponibile).
        assert_eq!(p(&format!("{m}\n\t/opt/my tools/claude\n")).as_deref(), Some("/opt/my tools/claude"));
        assert_eq!(p(&format!("{m}\n/usr/local/bin/claude\n")).as_deref(), Some("/usr/local/bin/claude"));
        // Niente marcatore (script non eseguito) o nessun candidato.
        assert_eq!(p("/home/u/.local/bin/claude\n"), None);
        assert_eq!(p(&format!("{m}\n")), None);
        assert_eq!(p(""), None);
        assert_eq!(p(&format!("{m}\nclaude is aliased to x\n")), None);
    }

    /// Regressione: in una distro senza claude, `command -v` trovava lo shim
    /// npm di Windows (PATH di Windows accodato da WSL), che avvia il
    /// `claude.exe` di Windows: plugin "installato" letto dalla config di
    /// Windows e `plugin validate /mnt/c/...` letto come `C:\mnt\c\...`.
    #[test]
    fn command_candidates_skip_windows_programs() {
        let p = |s: &str| parse_command_candidates(s);
        let m = CANDIDATES_MARKER;
        let shim = "/mnt/c/Users/j.magarelli/AppData/Roaming/npm/claude";
        // Solo il claude di Windows: nessun claude nella distro.
        assert_eq!(p(&format!("{m}\nv9fs\t{shim}\n")), None);
        // Per path anche senza tipo fs, e per tipo fs anche con mount root diversa.
        assert_eq!(p(&format!("{m}\n\t{shim}\n")), None);
        assert_eq!(p(&format!("{m}\n9p\t/c/Users/u/npm/claude\ndrvfs\t/win/claude\n")), None);
        // Windows prima nel PATH: vince comunque quello Linux.
        assert_eq!(
            p(&format!("{m}\nv9fs\t{shim}\next2/ext3\t/home/u/.local/bin/claude\n")).as_deref(),
            Some("/home/u/.local/bin/claude")
        );
        // Un drive con nome lungo non e' un drive Windows.
        assert_eq!(p(&format!("{m}\next2/ext3\t/mnt/cd/claude\n")).as_deref(), Some("/mnt/cd/claude"));
    }

    /// Risoluzione reale in una distro: i programmi di Windows (nel PATH via
    /// interop) non vengono mai scelti, quelli Linux si'.
    /// `cargo test -- --ignored resolve_wsl_real --nocapture`
    #[test]
    #[ignore]
    fn resolve_wsl_real_skips_windows_programs() {
        let distro = std::env::var("CC_TEST_DISTRO").unwrap_or_else(|_| "Ubuntu-24.04".to_string());
        let bash = resolve_wsl_command(&distro, "bash").expect("bash nella distro");
        assert!(bash.ends_with("/bash") && !is_windows_drive_path(&bash), "{bash}");
        // Solo nel PATH di Windows.
        assert_eq!(resolve_wsl_command(&distro, "notepad.exe"), None);
        assert_eq!(resolve_wsl_command(&distro, "cmd.exe"), None);
        let claude = resolve_wsl_command(&distro, "claude");
        println!("claude: {claude:?}");
        assert!(claude.as_deref().is_none_or(|p| !is_windows_drive_path(p)));
    }

    #[test]
    fn candidates_script_quotes_name() {
        let s = candidates_script("claude");
        assert!(s.starts_with(&format!("printf '%s\\n' {CANDIDATES_MARKER};")));
        assert!(s.contains("type -ap 'claude'"));
        assert!(candidates_script("it's").contains("type -ap 'it'\\''s'"));
    }
}
