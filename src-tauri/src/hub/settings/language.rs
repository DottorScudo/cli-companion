//! Lingua in cui i segretari rispondono all'utente. Il motore aggiunge al
//! system prompt "Always reply to the user in <english_name>"; i messaggi evento
//! sono testi dell'app e restano in italiano. Per una lingua nuova basta una
//! variante (e la riga in `ALL`): i file restano compatibili.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, rename = "HubLanguage")]
pub enum Language {
    #[default]
    It,
    En,
    Es,
    Fr,
    De,
}

impl Language {
    pub const ALL: [Language; 5] = [Self::It, Self::En, Self::Es, Self::Fr, Self::De];

    /// Codice sul filo e su disco (`it`, `en`...).
    pub fn code(self) -> &'static str {
        match self {
            Self::It => "it",
            Self::En => "en",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::De => "de",
        }
    }

    /// Nome inglese della lingua, per il prompt (lo legge il modello).
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the bot engine of the next stage"))]
    pub fn english_name(self) -> &'static str {
        match self {
            Self::It => "Italian",
            Self::En => "English",
            Self::Es => "Spanish",
            Self::Fr => "French",
            Self::De => "German",
        }
    }

    /// Lingua dal codice del frontend (senza distinguere le maiuscole).
    pub fn parse(raw: &str) -> Result<Self, String> {
        let code = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == code).ok_or_else(|| "lingua non supportata".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italian_is_the_default() {
        assert_eq!(Language::default(), Language::It);
        assert_eq!(Language::default().english_name(), "Italian");
    }

    #[test]
    fn codes_parse_and_match_serde() {
        for lang in Language::ALL {
            assert_eq!(Language::parse(lang.code()).unwrap(), lang);
            assert_eq!(serde_json::to_string(&lang).unwrap(), format!("\"{}\"", lang.code()));
            assert!(!lang.english_name().is_empty());
        }
        assert_eq!(Language::parse(" EN ").unwrap(), Language::En);
    }

    #[test]
    fn unknown_languages_are_rejected() {
        for bad in ["", "italian", "pt", "it-IT", "e n"] {
            assert!(Language::parse(bad).is_err(), "{bad}");
        }
    }
}
