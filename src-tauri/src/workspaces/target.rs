//! Il target (Windows o distro WSL) di un workspace.

use super::Workspace;
use crate::platform::{Target, TargetKind};

/// Errore di un workspace WSL senza distro.
pub(super) const WSL_DISTRO_MISSING: &str = "WSL distro missing";

impl Workspace {
    /// Dove girano i comandi del workspace. Errore se un workspace WSL non
    /// ha la distro (file modificato a mano).
    pub fn target(&self) -> Result<Target, String> {
        match self.kind {
            TargetKind::Windows => Ok(Target::Windows),
            TargetKind::Wsl => {
                let distro = self.distro.as_deref().filter(|d| !d.is_empty());
                distro.map(|d| Target::Wsl { distro: d.to_string() }).ok_or_else(|| WSL_DISTRO_MISSING.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(json: &str) -> Workspace {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn target_follows_kind_and_distro() {
        let windows = workspace(r#"{"id":"a","name":"a","kind":"windows","path":"C:\\a","created_at":1}"#);
        assert_eq!(windows.target(), Ok(Target::Windows));
        let wsl = workspace(r#"{"id":"b","name":"b","kind":"wsl","path":"/home/u","distro":"Ubuntu","created_at":1}"#);
        assert_eq!(wsl.target(), Ok(Target::Wsl { distro: "Ubuntu".to_string() }));
    }

    #[test]
    fn wsl_workspace_without_a_distro_has_no_target() {
        let missing = workspace(r#"{"id":"b","name":"b","kind":"wsl","path":"/home/u","created_at":1}"#);
        assert_eq!(missing.target(), Err(WSL_DISTRO_MISSING.to_string()));
        let empty = workspace(r#"{"id":"b","name":"b","kind":"wsl","path":"/home/u","distro":"","created_at":1}"#);
        assert_eq!(empty.target(), Err(WSL_DISTRO_MISSING.to_string()));
    }

    /// File scritto da una versione precedente: stesse stringhe in lettura e
    /// in scrittura, nessun campo in piu' o in meno per il `kind`.
    #[test]
    fn existing_workspaces_json_loads_and_saves_with_the_same_kind_strings() {
        let raw = r#"[
            {"id":"ws-1","name":"win","kind":"windows","path":"C:\\proj","created_at":1,"shell":"pwsh"},
            {"id":"ws-2","name":"lin","kind":"wsl","path":"/home/u/proj","distro":"Ubuntu-24.04","created_at":2}
        ]"#;
        let list: Vec<Workspace> = serde_json::from_str(raw).unwrap();
        assert_eq!((list[0].kind, list[1].kind), (TargetKind::Windows, TargetKind::Wsl));
        assert_eq!(list[1].distro.as_deref(), Some("Ubuntu-24.04"));

        let saved = serde_json::to_value(&list).unwrap();
        assert_eq!((saved[0]["kind"].as_str(), saved[1]["kind"].as_str()), (Some("windows"), Some("wsl")));
        assert_eq!(saved[0]["path"], "C:\\proj");
        assert_eq!(saved[1]["distro"], "Ubuntu-24.04");
        assert_eq!(serde_json::from_value::<Vec<Workspace>>(saved).unwrap(), list);
    }

    #[test]
    fn unknown_kind_is_rejected_when_parsing() {
        let raw = r#"[{"id":"x","name":"x","kind":"linux","path":"/x","created_at":1}]"#;
        assert!(serde_json::from_str::<Vec<Workspace>>(raw).is_err());
    }
}
