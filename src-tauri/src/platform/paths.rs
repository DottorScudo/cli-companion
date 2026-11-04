//! Conversioni di path fra Windows e le distro WSL.

/// Converte path Windows in path Linux WSL:
/// - `C:\Users\me\proj` -> `/mnt/c/Users/me/proj`
/// - `\\wsl$\Ubuntu\home\me` -> `/home/me`
pub fn windows_to_wsl_path(win: &str) -> Option<String> {
    let t = win.trim();
    let slashed = t.replace('\\', "/");
    let lower = slashed.to_lowercase();
    if lower.starts_with("//wsl$/") {
        let mut parts = slashed.split('/').filter(|p| !p.is_empty());
        let _ = parts.next()?; // wsl$
        let _ = parts.next()?; // distro
        let rest: Vec<&str> = parts.collect();
        return Some(format!("/{}", rest.join("/")));
    }
    let bytes = t.as_bytes();
    if t.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
        let drive = bytes[0] as char;
        if drive.is_ascii_alphabetic() {
            let rest = t[2..].replace('\\', "/");
            return Some(format!("/mnt/{}/{}", drive.to_ascii_lowercase(), rest.trim_start_matches('/')));
        }
    }
    None
}

/// `true` se il path Linux sta su un drive Windows montato (`/mnt/c/...`,
/// mount root predefinita): li' ci sono i programmi di Windows.
pub fn is_windows_drive_path(linux: &str) -> bool {
    let mut comps = linux.trim().split('/').filter(|c| !c.is_empty());
    linux.trim().starts_with('/')
        && comps.next() == Some("mnt")
        && comps.next().is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_alphabetic())
}

/// Inverso di `windows_to_wsl_path`: path Linux di `distro` -> path Windows
/// leggibile dall'host.
/// - `/mnt/c/Users/me` -> `C:\Users\me`
/// - `/home/me/x` -> `\\wsl.localhost\Ubuntu\home\me\x`
///
/// `None` se il path non e' assoluto.
pub fn wsl_to_windows_path(distro: &str, linux: &str) -> Option<String> {
    let t = linux.trim();
    if !t.starts_with('/') {
        return None;
    }
    let comps: Vec<&str> = t.split('/').filter(|c| !c.is_empty()).collect();
    if let [mnt, drive, rest @ ..] = comps.as_slice() {
        if *mnt == "mnt" && drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic() {
            let d = drive.to_ascii_uppercase();
            return Some(if rest.is_empty() { format!("{d}:\\") } else { format!("{d}:\\{}", rest.join("\\")) });
        }
    }
    Some(format!("\\\\wsl.localhost\\{distro}\\{}", comps.join("\\")))
}

/// Backslash -> slash e senza prefisso `\\?\` (i validatori degli agenti
/// rifiutano i backslash nei valori).
pub fn forward_slashes(p: &str) -> String {
    p.replace('\\', "/").trim_start_matches("//?/").to_string()
}

/// Lunghezza massima del nome di una distro.
const MAX_DISTRO_NAME_LEN: usize = 64;

/// Nome di distro sicuro da usare in un path UNC: niente separatori o
/// caratteri insoliti.
pub(crate) fn is_valid_distro_name(name: &str) -> bool {
    name.len() <= MAX_DISTRO_NAME_LEN
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_drive_paths_are_recognized() {
        for p in ["/mnt/c/Users/me/AppData/Roaming/npm/claude", "/mnt/D/x", "/mnt/c", " /mnt/c/x "] {
            assert!(is_windows_drive_path(p), "{p}");
        }
        for p in ["/home/u/.local/bin/claude", "/mnt/cd/x", "/mnt/wsl/x", "/mnt", "/usr/mnt/c/x", "mnt/c/x", ""] {
            assert!(!is_windows_drive_path(p), "{p}");
        }
    }

    #[test]
    fn wsl_to_windows_converts_mnt_and_distro_paths() {
        let w = |p: &str| wsl_to_windows_path("Ubuntu", p);
        assert_eq!(w("/mnt/c/Users/me/x.jsonl").as_deref(), Some(r"C:\Users\me\x.jsonl"));
        assert_eq!(w("/mnt/d").as_deref(), Some(r"D:\"));
        assert_eq!(
            w("/home/u/.claude/projects/p/s.jsonl").as_deref(),
            Some(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\p\s.jsonl")
        );
        // `/mnt/cd` non e' un drive.
        assert_eq!(w("/mnt/cd/x").as_deref(), Some(r"\\wsl.localhost\Ubuntu\mnt\cd\x"));
        assert_eq!(w("relativo/x"), None);
        assert_eq!(w(r"C:\x"), None);
        // Inverso di windows_to_wsl_path per i drive.
        let back = windows_to_wsl_path(&w("/mnt/c/a/b").unwrap()).unwrap();
        assert_eq!(back, "/mnt/c/a/b");
    }

    #[test]
    fn windows_to_wsl_converts_drives_and_unc_and_rejects_the_rest() {
        assert_eq!(windows_to_wsl_path(r"C:\Users\me\proj").as_deref(), Some("/mnt/c/Users/me/proj"));
        assert_eq!(windows_to_wsl_path("d:/x").as_deref(), Some("/mnt/d/x"));
        assert_eq!(windows_to_wsl_path(r"\\wsl$\Ubuntu\home\me").as_deref(), Some("/home/me"));
        assert_eq!(windows_to_wsl_path(r"\\wsl$"), None);
        assert_eq!(windows_to_wsl_path("relativo\\x"), None);
        assert_eq!(windows_to_wsl_path("/home/me"), None);
    }

    #[test]
    fn forward_slashes_strips_the_verbatim_prefix() {
        assert_eq!(forward_slashes(r"\\?\C:\a\b.exe"), "C:/a/b.exe");
    }

    #[test]
    fn distro_names_are_validated_for_unc_paths() {
        for ok in ["Ubuntu", "Ubuntu-24.04", "my_distro 2"] {
            assert!(is_valid_distro_name(ok), "{ok}");
        }
        for bad in ["..", "a/b", "a\\b", "x:y", &"a".repeat(65)] {
            assert!(!is_valid_distro_name(bad), "{bad}");
        }
    }
}
