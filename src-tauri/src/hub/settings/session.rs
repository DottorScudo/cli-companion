//! Parametri delle sessioni dei segretari: dopo quanti minuti di inattivita' la
//! conversazione interna si chiude e quanti passi di tool calling fa al massimo
//! un risveglio. Li applica il motore; qui il dato e i suoi limiti.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use ts_rs::TS;

/// Minuti di inattivita' ammessi: da un minuto a quattro ore.
pub const IDLE_MINUTES: RangeInclusive<u32> = 1..=240;
/// Passi di tool calling ammessi per risveglio.
pub const MAX_STEPS: RangeInclusive<u32> = 1..=20;
const DEFAULT_IDLE_MINUTES: u32 = 20;
const DEFAULT_MAX_STEPS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export, rename = "HubSessionSettings")]
pub struct SessionSettings {
    pub idle_minutes: u32,
    pub max_steps: u32,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self { idle_minutes: DEFAULT_IDLE_MINUTES, max_steps: DEFAULT_MAX_STEPS }
    }
}

impl SessionSettings {
    /// Valida i due valori con i limiti di `IDLE_MINUTES` e `MAX_STEPS`.
    pub fn new(idle_minutes: u32, max_steps: u32) -> Result<Self, String> {
        if !IDLE_MINUTES.contains(&idle_minutes) {
            return Err(format!(
                "la sessione deve chiudersi dopo {} a {} minuti di inattività",
                IDLE_MINUTES.start(),
                IDLE_MINUTES.end()
            ));
        }
        if !MAX_STEPS.contains(&max_steps) {
            return Err(format!("i passi per risveglio vanno da {} a {}", MAX_STEPS.start(), MAX_STEPS.end()));
        }
        Ok(Self { idle_minutes, max_steps })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_within_the_limits() {
        let d = SessionSettings::default();
        assert_eq!((d.idle_minutes, d.max_steps), (20, 8));
        assert_eq!(SessionSettings::new(d.idle_minutes, d.max_steps).unwrap(), d);
    }

    #[test]
    fn limits_are_inclusive() {
        assert!(SessionSettings::new(*IDLE_MINUTES.start(), *MAX_STEPS.start()).is_ok());
        assert!(SessionSettings::new(*IDLE_MINUTES.end(), *MAX_STEPS.end()).is_ok());
        assert!(SessionSettings::new(0, 8).is_err());
        assert!(SessionSettings::new(IDLE_MINUTES.end() + 1, 8).is_err());
        assert!(SessionSettings::new(20, 0).is_err());
        assert!(SessionSettings::new(20, MAX_STEPS.end() + 1).is_err());
    }

    #[test]
    fn missing_fields_take_their_default() {
        let s: SessionSettings = serde_json::from_str(r#"{"max_steps":3}"#).unwrap();
        assert_eq!(s, SessionSettings { idle_minutes: 20, max_steps: 3 });
    }
}
