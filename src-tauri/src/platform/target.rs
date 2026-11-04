//! Dove gira un comando: Windows o una distro WSL.

use super::paths::windows_to_wsl_path;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::paths::forward_slashes;

/// Chiave stabile (vista dal frontend) del target Windows.
pub const WINDOWS_KEY: &str = "windows";

/// Famiglia di un target. Forma persistita in `workspaces.json` e vista dal
/// frontend: `"windows"` / `"wsl"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Windows,
    Wsl,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Windows => "windows",
            TargetKind::Wsl => "wsl",
        }
    }

    /// Interpreta il valore ricevuto dal frontend (spazi e maiuscole ignorati).
    pub fn parse(value: &str) -> Result<TargetKind, String> {
        match value.trim().to_lowercase().as_str() {
            "windows" => Ok(TargetKind::Windows),
            "wsl" => Ok(TargetKind::Wsl),
            _ => Err("kind non valido (usa \"windows\" o \"wsl\")".to_string()),
        }
    }
}

/// Ambiente in cui gira un comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Windows,
    Wsl { distro: String },
}

impl Target {
    /// Chiave stabile usata dal frontend: "windows" o il nome della distro.
    pub fn key(&self) -> String {
        match self {
            Target::Windows => WINDOWS_KEY.to_string(),
            Target::Wsl { distro } => distro.clone(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Target::Windows => "Windows".to_string(),
            Target::Wsl { distro } => distro.clone(),
        }
    }

    pub fn kind(&self) -> TargetKind {
        match self {
            Target::Windows => TargetKind::Windows,
            Target::Wsl { .. } => TargetKind::Wsl,
        }
    }

    /// Inverso di `key`.
    pub fn from_key(key: &str) -> Result<Target, String> {
        let key = key.trim();
        if key.is_empty() {
            return Err("env mancante".to_string());
        }
        Ok(if key == WINDOWS_KEY { Target::Windows } else { Target::Wsl { distro: key.to_string() } })
    }

    /// Path di una cartella locale come lo vede il target: slash dritti su
    /// Windows, `/mnt/c/...` su WSL.
    pub fn path_arg(&self, win_path: &Path) -> Result<String, String> {
        let s = forward_slashes(&win_path.to_string_lossy());
        match self {
            Target::Windows => Ok(s),
            Target::Wsl { .. } => {
                windows_to_wsl_path(&s).ok_or_else(|| format!("percorso non traducibile per WSL: {s}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_roundtrip_and_reject_empty() {
        assert_eq!(Target::from_key("windows"), Ok(Target::Windows));
        assert_eq!(Target::from_key(" Ubuntu "), Ok(Target::Wsl { distro: "Ubuntu".to_string() }));
        assert!(Target::from_key("  ").is_err());
        for t in [Target::Windows, Target::Wsl { distro: "Ubuntu-24.04".to_string() }] {
            assert_eq!(Target::from_key(&t.key()).as_ref(), Ok(&t));
        }
    }

    #[test]
    fn kind_label_and_key_follow_the_target() {
        let wsl = Target::Wsl { distro: "U".to_string() };
        assert_eq!((Target::Windows.kind(), wsl.kind()), (TargetKind::Windows, TargetKind::Wsl));
        assert_eq!((Target::Windows.key().as_str(), Target::Windows.label().as_str()), ("windows", "Windows"));
        assert_eq!((wsl.key().as_str(), wsl.label().as_str()), ("U", "U"));
    }

    #[test]
    fn kind_parses_frontend_values_and_rejects_the_rest() {
        assert_eq!(TargetKind::parse(" WSL "), Ok(TargetKind::Wsl));
        assert_eq!(TargetKind::parse("windows"), Ok(TargetKind::Windows));
        assert!(TargetKind::parse("linux").is_err());
        assert!(TargetKind::parse("").is_err());
    }

    #[test]
    fn kind_serde_is_the_persisted_lowercase_string() {
        assert_eq!(serde_json::to_string(&TargetKind::Wsl).unwrap(), "\"wsl\"");
        assert_eq!(serde_json::from_str::<TargetKind>("\"windows\"").unwrap(), TargetKind::Windows);
        assert!(serde_json::from_str::<TargetKind>("\"linux\"").is_err());
        for kind in [TargetKind::Windows, TargetKind::Wsl] {
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{}\"", kind.as_str()));
        }
    }

    #[test]
    fn path_arg_is_native_on_windows_and_mounted_on_wsl() {
        let p = Path::new(r"C:\Users\me\AppData\Roaming\app");
        assert_eq!(Target::Windows.path_arg(p).unwrap(), "C:/Users/me/AppData/Roaming/app");
        let wsl = Target::Wsl { distro: "U".to_string() };
        assert_eq!(wsl.path_arg(p).unwrap(), "/mnt/c/Users/me/AppData/Roaming/app");
        assert!(wsl.path_arg(Path::new("relativo")).is_err());
    }
}
