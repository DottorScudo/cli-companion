//! Il backoff di un supervisore (docs/CONNECTORS.MD §7): esponenziale da 5 s a
//! 10 minuti, azzerato da un successo; `RateLimited` rispetta `retry_after` (al
//! massimo un'ora, mai meno del backoff corrente). Puro: la casualita' (jitter)
//! arriva dal chiamante come numero in [0, 1], cosi' si prova senza orologio.
//! Qui anche il parser dell'intestazione `Retry-After`, che i connettori usano
//! per costruire `ConnectorError::RateLimited`.

use crate::connectors::contracts::{Clock, ConnectorError};
use crate::connectors::limits::{BACKOFF_MAX, BACKOFF_MIN, RETRY_AFTER_MAX};
use std::time::Duration;

/// Quanto del tempo base il jitter puo' togliere: l'attesa sta in [75%, 100%] del base.
const JITTER_SPAN: f64 = 0.25;

#[derive(Debug, Default, Clone)]
pub struct Backoff {
    /// Il tempo base del prossimo fallimento (`None` = nessun fallimento di fila).
    next: Option<Duration>,
}

impl Backoff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un successo: si riparte da `BACKOFF_MIN`.
    pub fn reset(&mut self) {
        self.next = None;
    }

    /// Registra un fallimento e dice quanto aspettare. `jitter` e' in [0, 1]
    /// (fuori intervallo si riporta nei limiti).
    pub fn failure(&mut self, error: &ConnectorError, jitter: f64) -> Duration {
        let base = self.next.unwrap_or(BACKOFF_MIN).clamp(BACKOFF_MIN, BACKOFF_MAX);
        self.next = Some(base.saturating_mul(2).min(BACKOFF_MAX));
        let jitter = if jitter.is_finite() { jitter.clamp(0.0, 1.0) } else { 0.0 };
        let wait = base.mul_f64(1.0 - JITTER_SPAN * jitter);
        match error {
            ConnectorError::RateLimited { retry_after: Some(after) } => (*after).min(RETRY_AFTER_MAX).max(wait),
            _ => wait,
        }
    }
}

/// Un numero in [0, 1] dai byte casuali dell'orologio iniettato (0 se la
/// casualita' non e' disponibile: il backoff resta corretto, solo senza jitter).
pub fn jitter_from(clock: &dyn Clock) -> f64 {
    let mut bytes = [0u8; 2];
    match clock.fill_random(&mut bytes) {
        Ok(()) => f64::from(u16::from_le_bytes(bytes)) / f64::from(u16::MAX),
        Err(_) => 0.0,
    }
}

/// Il valore di `Retry-After`: un numero di secondi oppure una data HTTP
/// (RFC 2822/7231, es. `Wed, 21 Oct 2015 07:28:00 GMT`). `now_ms` e' l'ora
/// corrente (millisecondi Unix). Una data passata vale zero; un valore illeggibile `None`.
/// Il risultato non e' limitato: il tetto di un'ora lo applica `Backoff::failure`.
#[cfg_attr(not(test), expect(dead_code, reason = "lo usa un connettore che legge Retry-After: il registro e' vuoto"))]
pub fn parse_retry_after(raw: &str, now_ms: i64) -> Option<Duration> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 64 {
        return None;
    }
    if raw.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Duration::from_secs(raw.parse::<u64>().unwrap_or(u64::from(u32::MAX))));
    }
    let date = chrono::DateTime::parse_from_rfc2822(raw).ok()?;
    let wait_ms = date.timestamp_millis().saturating_sub(now_ms).max(0);
    Some(Duration::from_millis(u64::try_from(wait_ms).unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::testing::FakeClock;

    fn transient() -> ConnectorError {
        ConnectorError::Transient("x".to_string())
    }

    #[test]
    fn the_wait_doubles_from_5_seconds_up_to_10_minutes() {
        let mut backoff = Backoff::new();
        let waits: Vec<u64> = (0..10).map(|_| backoff.failure(&transient(), 0.0).as_secs()).collect();
        assert_eq!(waits, vec![5, 10, 20, 40, 80, 160, 320, 600, 600, 600]);
    }

    #[test]
    fn the_jitter_only_shortens_within_a_quarter() {
        for (jitter, expected) in [(0.0, 5.0), (1.0, 3.75), (0.5, 4.375)] {
            let wait = Backoff::new().failure(&transient(), jitter);
            assert!((wait.as_secs_f64() - expected).abs() < 1e-9, "{jitter}: {wait:?}");
        }
        // Fuori intervallo o NaN: ricondotto nei limiti, mai oltre il massimo.
        for jitter in [-3.0, 7.0, f64::NAN, f64::INFINITY] {
            let wait = Backoff::new().failure(&transient(), jitter);
            assert!(wait >= BACKOFF_MIN.mul_f64(0.75) && wait <= BACKOFF_MIN, "{jitter}: {wait:?}");
        }
        let mut backoff = Backoff::new();
        for _ in 0..20 {
            assert!(backoff.failure(&transient(), 0.0) <= BACKOFF_MAX);
        }
    }

    #[test]
    fn a_success_resets_the_sequence() {
        let mut backoff = Backoff::new();
        for _ in 0..4 {
            backoff.failure(&transient(), 0.0);
        }
        backoff.reset();
        assert_eq!(backoff.failure(&transient(), 0.0), BACKOFF_MIN);
    }

    #[test]
    fn retry_after_is_honoured_never_below_the_backoff_and_capped_at_one_hour() {
        let limited = |secs: Option<u64>| ConnectorError::RateLimited { retry_after: secs.map(Duration::from_secs) };
        let mut backoff = Backoff::new();
        assert_eq!(backoff.failure(&limited(Some(90)), 0.0), Duration::from_secs(90));
        // Retry-After piu' corto del backoff corrente (10 s): vale il backoff.
        assert_eq!(backoff.failure(&limited(Some(1)), 0.0), Duration::from_secs(10));
        // Senza retry_after si comporta come un errore transitorio.
        assert_eq!(backoff.failure(&limited(None), 0.0), Duration::from_secs(20));
        // Oltre un'ora: un'ora.
        assert_eq!(backoff.failure(&limited(Some(999_999)), 0.0), RETRY_AFTER_MAX);
    }

    #[test]
    fn retry_after_parses_seconds_and_http_dates() {
        let now = chrono::DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT").unwrap().timestamp_millis();
        assert_eq!(parse_retry_after("120", now), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after(" 0 ", now), Some(Duration::ZERO));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:30:00 GMT", now), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:00:00 GMT", now), Some(Duration::ZERO));
        for bad in ["", "abc", "-5", "1.5", "12 s", "Wed, 99 Foo 2015", &"9".repeat(80)] {
            assert_eq!(parse_retry_after(bad, now), None, "{bad:?}");
        }
        // Un numero enorme non manda in overflow.
        assert!(parse_retry_after(&"9".repeat(30), now).is_some());
    }

    #[test]
    fn the_jitter_comes_from_the_injected_clock_within_bounds() {
        let clock = FakeClock::new(0);
        for _ in 0..50 {
            let j = jitter_from(&clock);
            assert!((0.0..=1.0).contains(&j), "{j}");
        }
    }
}
