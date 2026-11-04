//! Contratto del pacchetto: i file che l'integrazione genera e il modo di
//! installarli nell'agente (per Claude Code: un plugin con gli hook).

use super::Detection;
use crate::platform::{windows_to_wsl_path, Target};
use std::path::{Path, PathBuf};

/// Contesto per installare/rimuovere un pacchetto.
pub struct InstallCtx {
    /// Exe corrente (e' il comando degli hook), slash dritti.
    pub exe: String,
    /// Cartella dati dell'app: qui vive il pacchetto generato.
    pub data_dir: PathBuf,
}

impl InstallCtx {
    /// Exe come lo vede l'agente nel target (`/mnt/c/...` su WSL).
    pub fn exe_for(&self, target: &Target) -> Result<String, String> {
        match target {
            Target::Windows => Ok(self.exe.clone()),
            Target::Wsl { .. } => windows_to_wsl_path(&self.exe)
                .ok_or_else(|| format!("percorso exe non traducibile per WSL: {}", self.exe)),
        }
    }

    /// Cartella del pacchetto generato per (agente, target). Il marketplace
    /// locale e' caricato sul posto: deve restare su disco finche' installato.
    pub fn plugin_root(&self, agent_id: &str, target: &Target) -> PathBuf {
        plugins_dir(&self.data_dir, agent_id).join(sanitize_env_key(&target.key()))
    }
}

/// Cartella dei pacchetti generati per un agente: una sottocartella per target.
pub fn plugins_dir(data_dir: &Path, agent_id: &str) -> PathBuf {
    data_dir.join("agent-plugins").join(agent_id)
}

/// Chiave del target sicura per nomi di cartella: solo ASCII semplici, resto `-`.
pub fn sanitize_env_key(key: &str) -> String {
    key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect()
}

/// FNV-1a a 32 bit: serve solo a derivare la versione di un pacchetto dal suo contenuto.
pub fn fnv32(s: &str) -> u32 {
    s.bytes().fold(0x811c9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x01000193))
}

/// Un file del pacchetto generato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    /// Relativo alla radice del pacchetto, slash dritti.
    pub path: String,
    /// UTF-8 senza BOM.
    pub content: String,
}

/// Pacchetto che l'app genera e installa nell'agente (hook, server MCP, skill).
pub trait IntegrationPackage: Sync {
    /// Versione che avrebbe il pacchetto generato ora per il target (`None` =
    /// non calcolabile per questo target: mai considerato da aggiornare).
    /// Solo calcolo, nessun processo.
    fn version(&self, ctx: &InstallCtx, target: &Target) -> Option<String>;

    /// Contenuto generato, per ispezione (conformita'). Puro: nessun
    /// processo, nessun accesso al disco.
    fn files(&self, ctx: &InstallCtx, target: &Target) -> Result<Vec<PackageFile>, String>;

    /// Solo se `det.found`: il pacchetto e' gia' installato?
    fn is_installed(&self, target: &Target, det: &Detection) -> bool;

    /// Installa (o reinstalla) il pacchetto; ritorna il log dei passi.
    fn install(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String>;

    /// Rimuove il pacchetto e i file generati; ritorna il log dei passi.
    fn uninstall(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv32_matches_the_published_test_vectors() {
        assert_eq!(fnv32(""), 0x811c9dc5);
        assert_eq!(fnv32("a"), 0xe40c292c);
        assert_eq!(fnv32("foobar"), 0xbf9cf968);
    }

    #[test]
    fn sanitize_env_key_keeps_simple_names() {
        assert_eq!(sanitize_env_key("windows"), "windows");
        assert_eq!(sanitize_env_key("Ubuntu-24.04"), "Ubuntu-24.04");
        assert_eq!(sanitize_env_key("a/b\\c d"), "a-b-c-d");
    }

    #[test]
    fn install_ctx_translates_exe_and_plugin_root_per_target() {
        let ctx = InstallCtx {
            exe: "C:/Program Files/cc/cli-companion.exe".to_string(),
            data_dir: PathBuf::from(r"C:\data"),
        };
        assert_eq!(
            ctx.exe_for(&Target::Wsl { distro: "U".into() }).unwrap(),
            "/mnt/c/Program Files/cc/cli-companion.exe"
        );
        let root = ctx.plugin_root("claude-code", &Target::Wsl { distro: "Ubuntu 24".into() });
        assert!(root.ends_with(Path::new("agent-plugins/claude-code/Ubuntu-24")));
    }
}
