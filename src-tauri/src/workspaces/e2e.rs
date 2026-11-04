//! Configurazione E2E di un repository: comando, report JUnit, timeout
//! (vedi docs/BOARD.MD, "Test E2E").

use super::validate::{MAX_REPO_COMMAND_LEN, MAX_REPO_PATH_LEN};
use crate::validate::{is_absolute_like, path_segments, ErrorCode, ValidationError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Timeout predefinito e limiti, in minuti.
pub const DEFAULT_E2E_TIMEOUT_MIN: u32 = 20;
pub const MIN_E2E_TIMEOUT_MIN: u32 = 1;
pub const MAX_E2E_TIMEOUT_MIN: u32 = 60;

fn default_timeout_min() -> u32 {
    DEFAULT_E2E_TIMEOUT_MIN
}

/// Come l'app esegue gli E2E di un repository: `command` nella cartella del
/// task, poi legge il report JUnit in `junit` (relativo alla cartella).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct E2eConfig {
    pub command: String,
    pub junit: String,
    #[serde(default = "default_timeout_min")]
    pub timeout_min: u32,
}

impl E2eConfig {
    /// Normalizza e valida: comando su una riga non vuoto, percorso JUnit
    /// relativo senza `..`, timeout nei limiti.
    pub fn normalized(self) -> Result<E2eConfig, ValidationError> {
        let command = self.command.trim();
        if command.is_empty() {
            return Err(ValidationError::new(ErrorCode::E2eCommandMissing));
        }
        if command.chars().any(|c| c.is_control() && c != '\t') {
            return Err(ValidationError::new(ErrorCode::E2eCommandMultiline));
        }
        if command.chars().count() > MAX_REPO_COMMAND_LEN {
            return Err(ValidationError::new(ErrorCode::E2eCommandTooLong).max(MAX_REPO_COMMAND_LEN));
        }
        if !(MIN_E2E_TIMEOUT_MIN..=MAX_E2E_TIMEOUT_MIN).contains(&self.timeout_min) {
            return Err(ValidationError::new(ErrorCode::E2eTimeoutInvalid)
                .min(MIN_E2E_TIMEOUT_MIN as usize)
                .max(MAX_E2E_TIMEOUT_MIN as usize));
        }
        Ok(E2eConfig {
            command: command.to_string(),
            junit: normalize_junit(&self.junit)?,
            timeout_min: self.timeout_min,
        })
    }
}

/// Percorso del report: relativo, `/` come separatore, senza `..`.
fn normalize_junit(raw: &str) -> Result<String, ValidationError> {
    let t = raw.trim().replace('\\', "/");
    if t.is_empty() {
        return Err(ValidationError::new(ErrorCode::E2eJunitMissing));
    }
    if is_absolute_like(&t) {
        return Err(ValidationError::new(ErrorCode::E2eJunitNotRelative));
    }
    if t.chars().count() > MAX_REPO_PATH_LEN || t.chars().any(char::is_control) {
        return Err(ValidationError::new(ErrorCode::E2eJunitInvalid).max(MAX_REPO_PATH_LEN));
    }
    let segs = path_segments(&t);
    if segs.is_empty() || segs.contains(&"..") {
        return Err(ValidationError::new(ErrorCode::E2eJunitEscapes));
    }
    Ok(segs.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(command: &str, junit: &str, timeout_min: u32) -> E2eConfig {
        E2eConfig { command: command.into(), junit: junit.into(), timeout_min }
    }

    #[test]
    fn is_normalized() {
        let c = cfg("  npm run e2e ", " .\\reports\\junit.xml ", 5).normalized().unwrap();
        assert_eq!(c, cfg("npm run e2e", "reports/junit.xml", 5));
    }

    #[test]
    fn timeout_defaults_when_missing() {
        let c: E2eConfig = serde_json::from_str(r#"{"command":"x","junit":"j.xml"}"#).unwrap();
        assert_eq!(c.timeout_min, DEFAULT_E2E_TIMEOUT_MIN);
        assert!(serde_json::from_str::<E2eConfig>(r#"{"command":"x","junit":"j","extra":1}"#).is_err());
    }

    #[test]
    fn rejections_carry_a_typed_code() {
        let code = |c: E2eConfig| c.normalized().unwrap_err().code();
        assert_eq!(code(cfg("  ", "j.xml", 5)), ErrorCode::E2eCommandMissing);
        assert_eq!(code(cfg("a\nb", "j.xml", 5)), ErrorCode::E2eCommandMultiline);
        assert_eq!(code(cfg(&"x".repeat(MAX_REPO_COMMAND_LEN + 1), "j.xml", 5)), ErrorCode::E2eCommandTooLong);
        assert_eq!(code(cfg("x", "j.xml", 0)), ErrorCode::E2eTimeoutInvalid);
        assert_eq!(code(cfg("x", " ", 5)), ErrorCode::E2eJunitMissing);
        assert_eq!(code(cfg("x", "C:/j", 5)), ErrorCode::E2eJunitNotRelative);
        assert_eq!(code(cfg("x", "../j", 5)), ErrorCode::E2eJunitEscapes);
        assert_eq!(code(cfg("x", "a\0b", 5)), ErrorCode::E2eJunitInvalid);
        let e = cfg("x", "j.xml", 99).normalized().unwrap_err();
        assert_eq!(e.message_it(), "timeout E2E non valido (da 1 a 60 minuti)");
        assert_eq!(e.message_en(), "invalid e2e timeout_min (from 1 to 60 minutes)");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(cfg("  ", "j.xml", 5).normalized().is_err());
        assert!(cfg("a\nb", "j.xml", 5).normalized().is_err());
        assert!(cfg(&"x".repeat(MAX_REPO_COMMAND_LEN + 1), "j.xml", 5).normalized().is_err());
        assert!(cfg(&"x".repeat(MAX_REPO_COMMAND_LEN), "j.xml", 5).normalized().is_ok());
        for j in ["", "  ", "../j.xml", "a/../../j", "/etc/j", "C:\\j.xml", "C:/j", "..", ".", "a\0b"] {
            assert!(cfg("x", j, 5).normalized().is_err(), "{j}");
        }
        assert!(cfg("x", &"a".repeat(MAX_REPO_PATH_LEN + 1), 5).normalized().is_err());
        assert!(cfg("x", &"a".repeat(MAX_REPO_PATH_LEN), 5).normalized().is_ok());
        for t in [0, MAX_E2E_TIMEOUT_MIN + 1] {
            assert!(cfg("x", "j.xml", t).normalized().is_err(), "{t}");
        }
        for t in [MIN_E2E_TIMEOUT_MIN, MAX_E2E_TIMEOUT_MIN] {
            assert!(cfg("x", "j.xml", t).normalized().is_ok(), "{t}");
        }
    }
}
