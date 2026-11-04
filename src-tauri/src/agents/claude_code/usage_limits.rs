//! Limiti dell'abbonamento di Claude Code (5 ore e settimanale).
//!
//! Stesso endpoint (non documentato) che usa `/usage`, chiamato con curl e il
//! token OAuth di `~/.claude/.credentials.json`. Il token non va mai sulla
//! riga di comando (l'header passa da stdin) e NON lo rinnoviamo noi: il
//! refresh ruoterebbe il refresh token e romperebbe il login di Claude Code.
//! Token scaduto = stato "expired" finche' Claude Code non lo rinnova.

use super::config_dir::windows_claude_dir;
use super::ClaudeCode;
use crate::agents::{LimitWindow, UsageLimits, UsageLimitsReply};
use crate::platform::Target;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Avviso mostrato accanto all'interruttore dei consumi.
const LIMITS_NOTICE: &str =
    "I limiti arrivano da un endpoint non documentato di Claude e potrebbero smettere di funzionare.";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// Un risultato valido si riusa per questo tempo.
const LIMITS_FRESH: Duration = Duration::from_secs(3 * 60);
/// Dopo un 429 non si richiama l'endpoint per questo tempo.
const LIMITS_BACKOFF: Duration = Duration::from_secs(10 * 60);

// ---------------------------------------------------------------------------
// Credenziali
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Creds {
    Token(String),
    Expired,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Legge `claudeAiOauth` dal JSON delle credenziali. `Err` = messaggio per
/// l'utente (nessun abbonamento / file illeggibile). Il token non finisce
/// mai nei messaggi.
fn parse_credentials(raw: &str, now_ms: u64) -> Result<Creds, String> {
    let v: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|_| "File delle credenziali di Claude Code non leggibile.".to_string())?;
    let oauth = v
        .get("claudeAiOauth")
        .filter(|o| o.is_object())
        .ok_or_else(|| "Nessun abbonamento Claude: i limiti non sono disponibili con una API key.".to_string())?;
    let token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Token di accesso di Claude Code assente: rifai il login.".to_string())?;
    let expires = oauth.get("expiresAt").and_then(|e| e.as_u64().or_else(|| e.as_f64().map(|f| f.max(0.0) as u64)));
    if expires.is_some_and(|e| e <= now_ms) {
        return Ok(Creds::Expired);
    }
    Ok(Creds::Token(token.to_string()))
}

/// File delle credenziali da provare, in ordine.
fn credentials_candidates(env: &Target) -> Result<Vec<PathBuf>, String> {
    match env {
        Target::Windows => Ok(vec![windows_claude_dir()?.join(".credentials.json")]),
        Target::Wsl { distro: d } => {
            let home = crate::wsl::linux_home(d)
                .map_err(|e| format!("Home della distro non risolta: {e}"))?
                .replace('/', "\\");
            Ok(["wsl.localhost", "wsl$"]
                .iter()
                .map(|host| PathBuf::from(format!("\\\\{host}\\{d}{home}\\.claude\\.credentials.json")))
                .collect())
        }
    }
}

fn read_credentials(env: &Target) -> Result<String, String> {
    let mut last = None;
    for path in credentials_candidates(env)? {
        match std::fs::read_to_string(&path) {
            Ok(raw) => return Ok(raw),
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) if e.kind() != std::io::ErrorKind::NotFound => format!("Credenziali di Claude Code non leggibili: {e}"),
        _ => "Credenziali di Claude Code non trovate: serve il login con un abbonamento.".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Limiti
// ---------------------------------------------------------------------------

fn parse_window(v: Option<&Value>) -> Option<LimitWindow> {
    let w = v?.as_object()?;
    let used = w.get("utilization")?.as_f64()?;
    Some(LimitWindow {
        used_percentage: used.max(0.0),
        resets_at: w.get("resets_at").and_then(Value::as_str).map(str::to_string),
    })
}

/// Interpreta la risposta dell'endpoint. Tollera campi mancanti o null;
/// `None` se non c'e' nessuna finestra riconoscibile.
fn parse_limits(body: &str) -> Option<UsageLimitsReply> {
    let v: Value = serde_json::from_str(body.trim_start_matches('\u{feff}').trim()).ok()?;
    let five_hour = parse_window(v.get("five_hour"));
    let seven_day = parse_window(v.get("seven_day"));
    if five_hour.is_none() && seven_day.is_none() {
        return None;
    }
    Some(UsageLimitsReply { state: "ok".to_string(), five_hour, seven_day, message: None })
}

/// Esito di una richiesta all'endpoint.
#[derive(Debug)]
enum Fetched {
    Ok(UsageLimitsReply),
    Expired,
    RateLimited,
    Failed(String),
}

/// Corpo e codice HTTP dall'output di curl (`corpo\n<codice>`).
fn split_status(out: &str) -> Option<(&str, u16)> {
    let (body, code) = out.trim_end().rsplit_once('\n')?;
    Some((body, code.trim().parse().ok()?))
}

fn classify_response(out: &str) -> Fetched {
    let Some((body, code)) = split_status(out) else {
        return Fetched::Failed("Risposta non riconosciuta.".to_string());
    };
    match code {
        200 => parse_limits(body)
            .map(Fetched::Ok)
            .unwrap_or_else(|| Fetched::Failed("Risposta non riconosciuta.".to_string())),
        401 | 403 => Fetched::Expired,
        429 => Fetched::RateLimited,
        c => Fetched::Failed(format!("Servizio non disponibile (HTTP {c}).")),
    }
}

fn fetch_limits_http(token: &str) -> Fetched {
    let args: Vec<String> = [
        "-s",
        "-m",
        "15",
        "-H",
        "@-",
        "-H",
        "anthropic-beta: oauth-2025-04-20",
        "-H",
        concat!("User-Agent: cli-companion/", env!("CARGO_PKG_VERSION")),
        "-w",
        "\n%{http_code}",
        USAGE_URL,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // L'header segreto passa da stdin: non compare nella lista dei processi.
    match crate::net::run_curl_stdin(&args, format!("Authorization: Bearer {token}\n").as_bytes()) {
        Ok(out) => classify_response(&String::from_utf8_lossy(&out)),
        Err(e) => Fetched::Failed(format!("Richiesta non riuscita: {e}")),
    }
}

fn unavailable(message: &str) -> UsageLimitsReply {
    UsageLimitsReply::status("unavailable", Some(message))
}

fn expired() -> UsageLimitsReply {
    UsageLimitsReply::status("expired", Some("Accesso scaduto: usa Claude Code per rinnovarlo."))
}

/// Cache dei limiti di un ambiente.
#[derive(Default)]
struct LimitsEntry {
    good: Option<(Instant, UsageLimitsReply)>,
    backoff_until: Option<Instant>,
}

impl LimitsEntry {
    fn fresh(&self, now: Instant) -> Option<UsageLimitsReply> {
        self.good.as_ref().filter(|(at, _)| now.saturating_duration_since(*at) < LIMITS_FRESH).map(|(_, r)| r.clone())
    }

    fn backing_off(&self, now: Instant) -> bool {
        self.backoff_until.is_some_and(|until| now < until)
    }

    /// Ultimo valore valido (anche vecchio), altrimenti "unavailable".
    fn fallback(&self, message: &str) -> UsageLimitsReply {
        self.good.as_ref().map(|(_, r)| r.clone()).unwrap_or_else(|| unavailable(message))
    }

    fn apply(&mut self, now: Instant, fetched: Fetched) -> UsageLimitsReply {
        match fetched {
            Fetched::Ok(reply) => {
                self.backoff_until = None;
                self.good = Some((now, reply.clone()));
                reply
            }
            Fetched::Expired => {
                self.good = None;
                expired()
            }
            Fetched::RateLimited => {
                self.backoff_until = Some(now + LIMITS_BACKOFF);
                self.fallback("Troppe richieste: riprovo tra qualche minuto.")
            }
            Fetched::Failed(m) => self.fallback(&m),
        }
    }
}

fn limits_cache() -> &'static Mutex<HashMap<String, LimitsEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, LimitsEntry>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Limiti dell'abbonamento per `env`, con cache (3 min) e pausa dopo un 429
/// (10 min). Bloccante: chiamare fuori dal main thread.
fn limits(env: &Target) -> UsageLimitsReply {
    let key = env.key();
    {
        let mut cache = limits_cache().lock().unwrap();
        let entry = cache.entry(key.clone()).or_default();
        let now = Instant::now();
        if let Some(r) = entry.fresh(now) {
            return r;
        }
        if entry.backing_off(now) {
            return entry.fallback("Troppe richieste: riprovo tra qualche minuto.");
        }
    }
    // Credenziali e rete fuori dal lock.
    let fetched = match read_credentials(env).and_then(|raw| parse_credentials(&raw, now_ms())) {
        Err(m) => return unavailable(&m),
        Ok(Creds::Expired) => Fetched::Expired,
        Ok(Creds::Token(t)) => fetch_limits_http(&t),
    };
    let mut cache = limits_cache().lock().unwrap();
    cache.entry(key).or_default().apply(Instant::now(), fetched)
}

impl UsageLimits for ClaudeCode {
    fn limits(&self, target: &Target) -> UsageLimitsReply {
        limits(target)
    }

    fn notice(&self) -> &'static str {
        LIMITS_NOTICE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- credenziali ---

    #[test]
    fn credentials_token_and_expiry() {
        let json = |exp: &str| {
            format!(r#"{{"claudeAiOauth":{{"accessToken":"tok","expiresAt":{exp},"subscriptionType":"max"}}}}"#)
        };
        assert_eq!(parse_credentials(&json("2000"), 1000), Ok(Creds::Token("tok".into())));
        assert_eq!(parse_credentials(&json("1000"), 1000), Ok(Creds::Expired));
        assert_eq!(parse_credentials(&json("500"), 1000), Ok(Creds::Expired));
        assert_eq!(parse_credentials(&json("2000.0"), 1000), Ok(Creds::Token("tok".into())));
        // expiresAt assente: non scaduto.
        let no_exp = r#"{"claudeAiOauth":{"accessToken":"tok"}}"#;
        assert_eq!(parse_credentials(no_exp, 1000), Ok(Creds::Token("tok".into())));
        // BOM tollerato.
        assert!(parse_credentials(&format!("\u{feff}{}", json("2000")), 1000).is_ok());
    }

    #[test]
    fn credentials_errors_never_leak_token() {
        for raw in [
            "",
            "non json",
            "{}",
            r#"{"claudeAiOauth":null}"#,
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
            r#"{"claudeAiOauth":{"expiresAt":5}}"#,
            r#"{"apiKey":"sk-secret"}"#,
        ] {
            let e = parse_credentials(raw, 0).unwrap_err();
            assert!(!e.contains("secret"), "{raw}");
        }
    }

    // --- limiti ---

    #[test]
    fn parse_limits_full_nulls_and_garbage() {
        let full = r#"{"five_hour":{"utilization":45.0,"resets_at":"2026-10-21T12:20:00.494382+00:00"},
            "seven_day":{"utilization":2,"resets_at":"2026-10-28T10:00:00.494406+00:00"},"seven_day_opus":null}"#;
        let r = parse_limits(full).unwrap();
        assert_eq!(r.state, "ok");
        let (f, s) = (r.five_hour.unwrap(), r.seven_day.unwrap());
        assert_eq!(f.used_percentage, 45.0);
        assert_eq!(f.resets_at.as_deref(), Some("2026-10-21T12:20:00.494382+00:00"));
        assert_eq!(s.used_percentage, 2.0);

        // Una finestra null / mancante: l'altra resta.
        let r = parse_limits(r#"{"five_hour":null,"seven_day":{"utilization":10.5,"resets_at":null}}"#).unwrap();
        assert!(r.five_hour.is_none());
        let s = r.seven_day.unwrap();
        assert_eq!((s.used_percentage, s.resets_at), (10.5, None));
        let r = parse_limits(r#"{"five_hour":{"utilization":1}}"#).unwrap();
        assert!(r.five_hour.is_some() && r.seven_day.is_none());

        // Niente di riconoscibile.
        for bad in [
            "",
            "garbage",
            "[]",
            "{}",
            r#"{"five_hour":null,"seven_day":null}"#,
            r#"{"five_hour":{"utilization":"x"}}"#,
            r#"{"five_hour":{"resets_at":"2026"}}"#,
            r#"{"five_hour":5}"#,
        ] {
            assert!(parse_limits(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn http_status_classification() {
        let body = r#"{"five_hour":{"utilization":3.0,"resets_at":null}}"#;
        assert!(matches!(classify_response(&format!("{body}\n200")), Fetched::Ok(_)));
        assert!(matches!(classify_response("{}\n401"), Fetched::Expired));
        assert!(matches!(classify_response("{}\n429"), Fetched::RateLimited));
        assert!(matches!(classify_response("x\n500"), Fetched::Failed(_)));
        assert!(matches!(classify_response("{}\n200"), Fetched::Failed(_)));
        assert!(matches!(classify_response("senza codice"), Fetched::Failed(_)));
        assert!(matches!(classify_response(""), Fetched::Failed(_)));
    }

    fn ok_reply(p: f64) -> UsageLimitsReply {
        UsageLimitsReply {
            state: "ok".into(),
            five_hour: Some(LimitWindow { used_percentage: p, resets_at: None }),
            seven_day: None,
            message: None,
        }
    }

    #[test]
    fn limits_cache_fresh_backoff_and_fallback() {
        let t0 = Instant::now();
        let mut e = LimitsEntry::default();
        assert!(e.fresh(t0).is_none());
        // 429 senza valore precedente: unavailable + pausa.
        let r = e.apply(t0, Fetched::RateLimited);
        assert_eq!(r.state, "unavailable");
        assert!(e.backing_off(t0 + Duration::from_secs(599)));
        assert!(!e.backing_off(t0 + Duration::from_secs(601)));
        // Successo: fresco per 3 minuti, poi no; la pausa si azzera.
        assert_eq!(e.apply(t0, Fetched::Ok(ok_reply(10.0))), ok_reply(10.0));
        assert!(!e.backing_off(t0));
        assert!(e.fresh(t0 + Duration::from_secs(179)).is_some());
        assert!(e.fresh(t0 + Duration::from_secs(181)).is_none());
        // 429 con valore precedente: ritorna l'ultimo buono.
        assert_eq!(e.apply(t0, Fetched::RateLimited), ok_reply(10.0));
        // Errore transitorio: idem. Scaduto: stato expired e valore scartato.
        assert_eq!(e.apply(t0, Fetched::Failed("x".into())), ok_reply(10.0));
        assert_eq!(e.apply(t0, Fetched::Expired).state, "expired");
        assert_eq!(e.apply(t0, Fetched::Failed("x".into())).state, "unavailable");
    }
}
