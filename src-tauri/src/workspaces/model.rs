//! Il modello del workspace: identita' e posizione, avvii, impostazioni dei
//! worker, consegna, repository. Il JSON (file dei dati e forma inviata al
//! frontend) e' piatto: `WorkerSettings` ci entra con `#[serde(flatten)]`.

use super::delivery::Delivery;
use super::repo_config::RepoConfig;
use super::worker_settings::WorkerSettings;
use crate::platform::TargetKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Avvio personalizzato di un workspace: comando eseguito all'apertura di un
/// nuovo terminale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Launch {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    /// Famiglia del target: `"windows"` oppure `"wsl"` nel JSON.
    pub kind: TargetKind,
    /// Percorso Windows (es. C:\proj) oppure percorso Linux assoluto (es. /home/user/proj)
    pub path: String,
    /// Distro WSL, valorizzato solo quando kind è Wsl
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub distro: Option<String>,
    /// Shell opzionale (es. pwsh, cmd, bash, zsh). Default: powershell su Windows, shell di default su WSL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shell: Option<String>,
    pub created_at: i64,
    /// Avvii personalizzati del workspace (vuoto = solo shell semplice).
    #[serde(default)]
    pub launches: Vec<Launch>,
    /// Agente dei worker e dei revisori del workspace: `None` = il predefinito
    /// globale. Vedi `agents::registry::resolve_agent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    /// Impostazioni dei worker automatici.
    #[serde(flatten)]
    pub worker: WorkerSettings,
    /// Modalita' di consegna dei task (vedi docs/BOARD.MD).
    #[serde(default)]
    pub delivery: Delivery,
    /// I task di questo workspace partono solo nelle fasce orarie (vedi `schedule`).
    #[serde(default)]
    pub use_schedule: bool,
    /// Avvio predefinito dei nuovi terminali: `None` = eredita il predefinito
    /// globale; "shell" = shell semplice; altrimenti l'id di un avvio del
    /// workspace o di uno integrato (`builtin:...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default_launch: Option<String>,
    /// Configurazione per repository (chiave = cartella relativa alla root,
    /// "." = la root): branch base, file ignorati da copiare e comandi di
    /// setup / pulizia dei worktree, eseguiti dall'app.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub repos: BTreeMap<String, RepoConfig>,
    /// Archiviato: congelato (nessun task, nessun tab);
    /// i dati restano e si puo' riattivare.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
}

impl Workspace {
    /// Workspace con le impostazioni di fabbrica: nessun avvio, worker al
    /// permesso di default, consegna di default, non archiviato.
    pub fn new(id: String, name: String, kind: TargetKind, path: String) -> Self {
        Self {
            id,
            name,
            kind,
            path,
            distro: None,
            shell: None,
            created_at: 0,
            launches: Vec::new(),
            agent: None,
            worker: WorkerSettings::default(),
            delivery: Delivery::default(),
            use_schedule: false,
            default_launch: None,
            repos: BTreeMap::new(),
            archived: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::PermissionMode;
    use crate::workspaces::worker_settings::AgentWorkerSettings;
    use serde_json::json;

    const MINIMAL: &str = r#"{"id":"ws-1","name":"p","kind":"windows","path":"C:\\p","created_at":1}"#;

    fn minimal() -> Workspace {
        serde_json::from_str(MINIMAL).unwrap()
    }

    #[test]
    fn a_minimal_workspace_gets_the_historic_defaults() {
        let ws = minimal();
        assert!(ws.launches.is_empty() && ws.repos.is_empty() && !ws.archived && !ws.use_schedule);
        assert_eq!(ws.worker, WorkerSettings::default());
        assert_eq!(ws.worker.permission, PermissionMode::AcceptEdits);
        assert!(ws.agent.is_none() && ws.worker.agents.is_empty());
        assert_eq!(
            ws,
            Workspace {
                created_at: 1,
                ..Workspace::new("ws-1".into(), "p".into(), TargetKind::Windows, "C:\\p".into())
            }
        );
    }

    /// La forma inviata al frontend e salvata su disco non cambia: i campi dei
    /// worker restano al livello del workspace, con il prefisso `worker_`.
    #[test]
    fn worker_settings_stay_flat_in_the_json() {
        let mut ws = minimal();
        ws.worker = WorkerSettings {
            permission: PermissionMode::Plan,
            rules: "no tests".into(),
            agents: BTreeMap::from([(
                "agent-a".to_string(),
                AgentWorkerSettings {
                    model: Some("big".into()),
                    allow: vec!["Read".into()],
                    deny: vec!["WebFetch".into()],
                },
            )]),
        };
        ws.agent = Some("agent-a".into());
        let v = serde_json::to_value(&ws).unwrap();
        assert_eq!(v["agent"], "agent-a");
        assert_eq!(v["worker_permission"], "plan");
        assert_eq!(v["worker_agents"], json!({"agent-a": {"model": "big", "allow": ["Read"], "deny": ["WebFetch"]}}));
        assert_eq!(v["worker_rules"], "no tests");
        for legacy in ["worker_model", "worker_allow", "worker_deny"] {
            assert!(v.get(legacy).is_none(), "{legacy}");
        }
        assert!(v.get("worker").is_none());
        assert_eq!(serde_json::from_value::<Workspace>(v).unwrap(), ws);
    }

    #[test]
    fn the_default_workspace_serializes_compactly() {
        let v = serde_json::to_value(minimal()).unwrap();
        assert_eq!(
            v,
            json!({
                "id": "ws-1", "name": "p", "kind": "windows", "path": "C:\\p", "created_at": 1,
                "launches": [], "worker_permission": "accept-edits", "worker_agents": {},
                "delivery": {"mode": "commit", "parallel": false, "merge_method": "squash"},
                "use_schedule": false
            })
        );
    }

    #[test]
    fn an_unknown_permission_falls_back_without_losing_the_workspace() {
        let raw =
            r#"{"id":"ws-1","name":"p","kind":"windows","path":"C:\\p","created_at":1,"worker_permission":"boh"}"#;
        let ws: Workspace = serde_json::from_str(raw).unwrap();
        assert_eq!(ws.worker.permission, PermissionMode::AcceptEdits);
        for mode in PermissionMode::ALL {
            let raw = format!(
                r#"{{"id":"a","name":"a","kind":"windows","path":"C:\\a","created_at":1,"worker_permission":"{}"}}"#,
                mode.as_str()
            );
            assert_eq!(serde_json::from_str::<Workspace>(&raw).unwrap().worker.permission, mode);
        }
    }

    #[test]
    fn optional_fields_are_written_only_when_set() {
        let mut ws = minimal();
        ws.archived = true;
        let raw = serde_json::to_string(&ws).unwrap();
        assert!(raw.contains("\"archived\":true"));
        assert!(serde_json::from_str::<Workspace>(&raw).unwrap().archived);
        let plain = serde_json::to_string(&minimal()).unwrap();
        for absent in ["archived", "repos", "default_launch", "worker_rules", "\"agent\"", "distro", "shell"] {
            assert!(!plain.contains(absent), "{absent}");
        }
    }

    #[test]
    fn an_old_launch_with_is_default_is_still_readable() {
        let raw = r#"{"id":"l1","name":"n","command":"ls","is_default":true}"#;
        let launch: Launch = serde_json::from_str(raw).unwrap();
        assert!(!serde_json::to_string(&launch).unwrap().contains("is_default"));
    }
}
