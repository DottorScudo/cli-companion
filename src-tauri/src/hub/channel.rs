//! Il canale di un thread: il Generale oppure un workspace. Sul filo e su disco
//! e' una stringa semplice: `"general"` oppure l'id del workspace.

use crate::validate::valid_workspace_id;
use serde::{Deserialize, Serialize};

/// Nome del canale Generale sul filo e del suo file (`threads/general.json`).
pub const GENERAL: &str = "general";

/// Lunghezza massima dell'id di un workspace usato come canale (in byte): i
/// nomi dei file restano corti e l'id non serve oltre.
const WORKSPACE_ID_MAX: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Channel {
    General,
    Workspace(String),
}

impl Channel {
    /// Canale da una stringa del frontend o del file. Un workspace non puo'
    /// chiamarsi `general` (il nome e' riservato, anche nel file) e il suo id
    /// deve essere un nome di file sicuro.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw == GENERAL {
            return Ok(Self::General);
        }
        if raw.len() > WORKSPACE_ID_MAX || !valid_workspace_id(raw) {
            return Err("canale non valido".to_string());
        }
        Ok(Self::Workspace(raw.to_string()))
    }

    /// Nome del file dei thread del canale (nella cartella `threads/`).
    pub fn file_name(&self) -> String {
        format!("{}.json", self.as_str())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::General => GENERAL,
            Self::Workspace(id) => id,
        }
    }
}

impl TryFrom<String> for Channel {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, String> {
        Self::parse(&raw)
    }
}

impl From<Channel> for String {
    fn from(channel: Channel) -> Self {
        channel.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_and_workspaces_parse() {
        assert_eq!(Channel::parse("general").unwrap(), Channel::General);
        assert_eq!(Channel::parse("ws-1-2").unwrap(), Channel::Workspace("ws-1-2".to_string()));
        assert_eq!(Channel::General.file_name(), "general.json");
        assert_eq!(Channel::parse("ws-1").unwrap().file_name(), "ws-1.json");
    }

    #[test]
    fn unsafe_ids_are_rejected() {
        for bad in ["", "..", "../x", "a/b", "a\\b", "a.json", "a b", &"a".repeat(WORKSPACE_ID_MAX + 1)] {
            assert!(Channel::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Channel::parse(&"a".repeat(WORKSPACE_ID_MAX)).is_ok());
    }

    #[test]
    fn serializes_as_a_plain_string() {
        let ch = Channel::Workspace("ws-1".to_string());
        assert_eq!(serde_json::to_string(&ch).unwrap(), "\"ws-1\"");
        assert_eq!(serde_json::to_string(&Channel::General).unwrap(), "\"general\"");
        assert_eq!(serde_json::from_str::<Channel>("\"general\"").unwrap(), Channel::General);
        assert!(serde_json::from_str::<Channel>("\"../x\"").is_err());
    }
}
