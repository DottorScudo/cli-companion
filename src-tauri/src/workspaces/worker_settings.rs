//! Impostazioni dei worker automatici di un workspace. Quelle che dipendono
//! dall'agente (modello, regole di permesso: ognuno ha i suoi modelli e la sua
//! sintassi) stanno in `worker_agents`, una voce per agente; il resto
//! (tetto dei permessi, regole di testo) vale per qualunque agente.

use crate::agents::PermissionMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Tetto dei permessi dei worker per i workspace nuovi e per i valori
/// assenti o illeggibili nel file: mai piu' permissivo del solito.
pub const DEFAULT_WORKER_PERMISSION: PermissionMode = PermissionMode::AcceptEdits;

/// Impostazioni dei worker per un agente.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AgentWorkerSettings {
    /// Modello di default (None = quello dell'agente); vale quando il task non ne indica uno.
    pub model: Option<String>,
    /// Regole di permesso pre-approvate, nella sintassi dell'agente (vedi
    /// `agents::worker_rules`).
    pub allow: Vec<String>,
    /// Regole di permesso vietate.
    pub deny: Vec<String>,
}

impl AgentWorkerSettings {
    /// Nessun modello e nessuna regola: la voce non serve conservarla.
    pub fn is_empty(&self) -> bool {
        self.model.is_none() && self.allow.is_empty() && self.deny.is_empty()
    }
}

/// Impostazioni dei worker automatici del workspace. I nomi nel JSON
/// conservano il prefisso `worker_`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WorkerSettings {
    /// Tetto dei permessi: `"plan"` | `"default"` | `"accept-edits"` | `"bypass"` nel JSON.
    #[serde(rename = "worker_permission", default = "default_permission", with = "permission_format")]
    // `permission_format` scrive il testo storico, che e' la forma di `PermissionMode` per ts-rs.
    #[ts(as = "PermissionMode")]
    pub permission: PermissionMode,
    /// Regole per i worker (test vietati, comandi da non lanciare): `get_task`
    /// le accoda alle istruzioni di ogni worker.
    #[serde(rename = "worker_rules", default, skip_serializing_if = "String::is_empty")]
    pub rules: String,
    /// Modello e regole di permesso per agente (chiave = id dell'integrazione).
    /// Sempre scritto, anche vuoto.
    #[serde(rename = "worker_agents", default)]
    pub agents: BTreeMap<String, AgentWorkerSettings>,
}

impl Default for WorkerSettings {
    fn default() -> Self {
        Self { permission: DEFAULT_WORKER_PERMISSION, rules: String::new(), agents: BTreeMap::new() }
    }
}

impl WorkerSettings {
    /// Le impostazioni dell'agente `id`, se ne ha.
    pub fn for_agent(&self, id: &str) -> Option<&AgentWorkerSettings> {
        self.agents.get(id)
    }

    /// Modello di default dell'agente `id` (None = quello dell'agente).
    pub fn model_for(&self, id: &str) -> Option<&str> {
        self.for_agent(id)?.model.as_deref()
    }

    /// Regole `(allow, deny)` dell'agente `id`; vuote se non ne ha.
    pub fn rules_for(&self, id: &str) -> (&[String], &[String]) {
        self.for_agent(id).map_or((&[], &[]), |s| (s.allow.as_slice(), s.deny.as_slice()))
    }

    /// Imposta le impostazioni dell'agente `id`; una voce vuota viene tolta.
    pub fn set_agent(&mut self, id: &str, settings: AgentWorkerSettings) {
        if settings.is_empty() {
            self.agents.remove(id);
        } else {
            self.agents.insert(id.to_string(), settings);
        }
    }
}

fn default_permission() -> PermissionMode {
    DEFAULT_WORKER_PERMISSION
}

/// Il permesso nel JSON e' il testo storico; un valore sconosciuto vale il
/// default (non rende illeggibile l'intero workspace).
mod permission_format {
    use super::{PermissionMode, DEFAULT_WORKER_PERMISSION};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(mode: &PermissionMode, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(mode.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PermissionMode, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(PermissionMode::parse(&raw).unwrap_or_else(|| {
            log::warn!(
                "permesso dei worker sconosciuto nel file ({raw:?}): uso {}",
                DEFAULT_WORKER_PERMISSION.as_str()
            );
            DEFAULT_WORKER_PERMISSION
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(model: Option<&str>, allow: &[&str], deny: &[&str]) -> AgentWorkerSettings {
        AgentWorkerSettings {
            model: model.map(str::to_string),
            allow: allow.iter().map(|s| s.to_string()).collect(),
            deny: deny.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn lookups_are_empty_for_an_agent_without_settings() {
        let worker = WorkerSettings::default();
        assert!(worker.for_agent("a").is_none());
        assert_eq!(worker.model_for("a"), None);
        assert_eq!(worker.rules_for("a"), (&[][..], &[][..]));
    }

    #[test]
    fn lookups_read_the_entry_of_the_agent_only() {
        let mut worker = WorkerSettings::default();
        worker.set_agent("a", settings(Some("m1"), &["Read"], &["Edit"]));
        worker.set_agent("b", settings(Some("m2"), &[], &[]));
        assert_eq!(worker.model_for("a"), Some("m1"));
        assert_eq!(worker.model_for("b"), Some("m2"));
        assert_eq!(worker.rules_for("a"), (&["Read".to_string()][..], &["Edit".to_string()][..]));
        assert_eq!(worker.rules_for("b"), (&[][..], &[][..]));
    }

    #[test]
    fn an_empty_entry_is_removed_instead_of_stored() {
        let mut worker = WorkerSettings::default();
        worker.set_agent("a", settings(None, &["Read"], &[]));
        assert!(worker.agents.contains_key("a"));
        worker.set_agent("a", AgentWorkerSettings::default());
        assert!(worker.agents.is_empty());
        worker.set_agent("never", AgentWorkerSettings::default());
        assert!(worker.agents.is_empty());
    }

    #[test]
    fn a_partial_agent_entry_in_the_file_gets_the_defaults() {
        let parsed: AgentWorkerSettings = serde_json::from_str(r#"{"allow":["Read"]}"#).unwrap();
        assert_eq!(parsed, settings(None, &["Read"], &[]));
    }
}
