//! Il provider del modello dei segretari: un solo endpoint compatibile OpenAI per
//! tutta l'app (`base_url`, es. `http://127.0.0.1:8080/v1`) e un solo modello.
//! Vuoti = non configurato. La API key sta nel portachiavi (`hub::api_key`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lunghezza massima dell'indirizzo, in caratteri.
pub const BASE_URL_MAX: usize = 2048;
/// Lunghezza massima del nome del modello, in caratteri.
pub const MODEL_MAX: usize = 128;
const SCHEMES: [&str; 2] = ["http://", "https://"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export, rename = "HubProvider")]
pub struct Provider {
    /// Indirizzo base dell'API, senza `/` finale; vuoto = non configurato.
    pub base_url: String,
    /// Nome del modello; vuoto = non configurato.
    pub model: String,
}

impl Provider {
    /// Valida i due campi; ognuno puo' essere vuoto (non ancora configurato).
    pub fn new(base_url: &str, model: &str) -> Result<Self, String> {
        Ok(Self { base_url: normalize_base_url(base_url)?, model: normalize_model(model)? })
    }

    /// Indirizzo e modello presenti: il motore puo' chiamare il modello.
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the bot engine of the next stage"))]
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.model.is_empty()
    }
}

/// `http://` o `https://` (schema in minuscolo), un host, una porta facoltativa
/// valida, nessuna credenziale (`user@`), frammento, spazio o carattere di
/// controllo. Toglie i `/` finali. Vuoto resta vuoto.
fn normalize_base_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Ok(String::new());
    }
    if url.chars().count() > BASE_URL_MAX {
        return Err(format!("l'indirizzo supera i {BASE_URL_MAX} caratteri"));
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("l'indirizzo contiene spazi o caratteri non validi".to_string());
    }
    let lower = url.to_ascii_lowercase();
    let scheme = SCHEMES
        .into_iter()
        .find(|s| lower.starts_with(s))
        .ok_or_else(|| "l'indirizzo deve iniziare con http:// o https://".to_string())?;
    let rest = &url[scheme.len()..];
    if rest.contains('#') {
        return Err("l'indirizzo non può contenere un frammento (#)".to_string());
    }
    let authority = rest.split(['/', '?']).next().unwrap_or_default();
    check_authority(authority)?;
    Ok(format!("{scheme}{}", rest.trim_end_matches('/')))
}

/// Host (nome, IPv4 o IPv6 tra parentesi quadre) con porta facoltativa.
fn check_authority(authority: &str) -> Result<(), String> {
    if authority.contains('@') {
        return Err("l'indirizzo non può contenere credenziali: usa la API key".to_string());
    }
    let (host, port) = split_port(authority)?;
    let valid_host = match host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        Some(ipv6) => !ipv6.is_empty() && ipv6.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.'),
        None => {
            !host.is_empty()
                && !host.starts_with(['.', '-'])
                && host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
        }
    };
    if !valid_host {
        return Err("l'indirizzo non ha un host valido".to_string());
    }
    match port {
        Some(p) if !p.bytes().all(|b| b.is_ascii_digit()) || !matches!(p.parse::<u16>(), Ok(n) if n > 0) => {
            Err("la porta dell'indirizzo non è valida".to_string())
        }
        _ => Ok(()),
    }
}

/// Separa `host:porta`, tenendo conto delle parentesi quadre di IPv6.
fn split_port(authority: &str) -> Result<(&str, Option<&str>), String> {
    let host_end = if authority.starts_with('[') {
        authority.find(']').map(|i| i + 1).ok_or_else(|| "l'indirizzo non ha un host valido".to_string())?
    } else {
        authority.find(':').unwrap_or(authority.len())
    };
    let (host, tail) = authority.split_at(host_end);
    match tail.strip_prefix(':') {
        Some(port) => Ok((host, Some(port))),
        None if tail.is_empty() => Ok((host, None)),
        None => Err("l'indirizzo non ha un host valido".to_string()),
    }
}

/// Trim, al massimo `MODEL_MAX` caratteri, niente caratteri di controllo.
fn normalize_model(raw: &str) -> Result<String, String> {
    let model = raw.trim();
    if model.chars().count() > MODEL_MAX {
        return Err(format!("il nome del modello supera i {MODEL_MAX} caratteri"));
    }
    if model.chars().any(char::is_control) {
        return Err("il nome del modello contiene caratteri non validi".to_string());
    }
    Ok(model.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> Result<String, String> {
        Provider::new(raw, "m").map(|p| p.base_url)
    }

    #[test]
    fn empty_means_not_configured() {
        let p = Provider::new("  ", "").unwrap();
        assert_eq!(p, Provider::default());
        assert!(!p.is_configured());
        assert!(!Provider::new("http://localhost/v1", "").unwrap().is_configured());
        assert!(!Provider::new("", "llama").unwrap().is_configured());
        assert!(Provider::new("http://localhost/v1", "llama").unwrap().is_configured());
    }

    #[test]
    fn common_endpoints_are_accepted_and_normalized() {
        assert_eq!(url("http://127.0.0.1:8080/v1").unwrap(), "http://127.0.0.1:8080/v1");
        assert_eq!(url(" HTTPS://api.deepseek.com/v1/ ").unwrap(), "https://api.deepseek.com/v1");
        assert_eq!(url("http://localhost:11434/v1").unwrap(), "http://localhost:11434/v1");
        assert_eq!(url("http://[::1]:8080").unwrap(), "http://[::1]:8080");
        assert_eq!(url("https://host/v1?api-version=2").unwrap(), "https://host/v1?api-version=2");
        assert_eq!(url("https://my_host.local").unwrap(), "https://my_host.local");
    }

    #[test]
    fn bad_urls_are_rejected() {
        for bad in [
            "ftp://host",
            "localhost:8080",
            "http://",
            "http:///v1",
            "http://host name/v1",
            "http://host/v 1",
            "http://host/\u{7}",
            "http://user:pw@host/v1",
            "http://host:0",
            "http://host:65536",
            "http://host:80a",
            "http://host:",
            "http://[::1",
            "http://[]",
            "http://[::1]x",
            "http://.host",
            "http://-host",
            "http://host/v1#x",
            "javascript:alert(1)",
        ] {
            assert!(url(bad).is_err(), "{bad}");
        }
        assert!(url(&format!("http://h/{}", "a".repeat(BASE_URL_MAX))).is_err());
    }

    #[test]
    fn model_limits() {
        assert_eq!(Provider::new("", " qwen2.5-coder:7b ").unwrap().model, "qwen2.5-coder:7b");
        assert!(Provider::new("", &"m".repeat(MODEL_MAX)).is_ok());
        assert!(Provider::new("", &"m".repeat(MODEL_MAX + 1)).is_err());
        assert!(Provider::new("", &"é".repeat(MODEL_MAX)).is_ok());
        assert!(Provider::new("", "a\nb").is_err());
    }
}
