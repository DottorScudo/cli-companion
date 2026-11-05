//! Abbinamento di un account esterno a un'istanza di contatto (docs/CONNECTORS.MD
//! §10): codici monouso di 12 caratteri da un alfabeto senza ambiguita', validi 10
//! minuti, solo in memoria. Al piu' 5 tentativi ogni 10 minuti per istanza: al
//! raggiungimento del limite i codici in attesa vengono annullati e nessun
//! codice, nemmeno quello giusto, e' piu' accettato finche' la finestra non passa.
//! Puro: ora e casualita' arrivano da `Clock`.

use crate::connectors::contracts::Clock;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

pub const CODE_LEN: usize = 12;
/// 32 simboli (potenza di 2: nessun bias nel mod): niente 0/O, 1/I/L.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789_";
pub const CODE_TTL_MS: i64 = 10 * 60 * 1000;
pub const ATTEMPT_WINDOW_MS: i64 = 10 * 60 * 1000;
pub const ATTEMPTS_MAX: usize = 5;
/// Codici in attesa per istanza.
const PENDING_MAX: usize = 5;

/// Come e' andato un tentativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairAttempt {
    Accepted,
    Rejected,
    TooManyAttempts,
}

#[derive(Debug, Default)]
pub struct PairingCodes {
    /// (codice, scadenza in ms).
    pending: Vec<(String, i64)>,
    /// Istanti dei tentativi nella finestra.
    attempts: Vec<i64>,
}

fn normalize(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

impl PairingCodes {
    /// Un codice nuovo; i piu' vecchi escono se ce ne sono troppi in attesa.
    pub fn issue(&mut self, clock: &dyn Clock) -> Result<String, String> {
        let now = clock.now_ms();
        self.pending.retain(|(_, expires)| *expires > now);
        let mut bytes = [0u8; CODE_LEN];
        clock.fill_random(&mut bytes)?;
        let code: String = bytes.iter().map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()])).collect();
        self.pending.push((code.clone(), now.saturating_add(CODE_TTL_MS)));
        while self.pending.len() > PENDING_MAX {
            self.pending.remove(0);
        }
        Ok(code)
    }

    /// Un account ha presentato `code`. Un codice valido si consuma.
    pub fn attempt(&mut self, code: &str, now: i64) -> PairAttempt {
        self.attempts.retain(|at| now.saturating_sub(*at) < ATTEMPT_WINDOW_MS);
        self.pending.retain(|(_, expires)| *expires > now);
        if self.attempts.len() >= ATTEMPTS_MAX {
            self.pending.clear();
            return PairAttempt::TooManyAttempts;
        }
        self.attempts.push(now);
        let code = normalize(code);
        let found = self.pending.iter().position(|(pending, _)| *pending == code);
        if let Some(index) = found {
            self.pending.remove(index);
            return PairAttempt::Accepted;
        }
        if self.attempts.len() >= ATTEMPTS_MAX {
            self.pending.clear();
        }
        PairAttempt::Rejected
    }

    #[cfg(test)]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// I codici di tutte le istanze, condivisi fra il ciclo di contatto (che li
/// consuma) e il runtime (che li emette per l'azione `pair`).
#[derive(Default)]
pub struct PairingRegistry {
    instances: Mutex<HashMap<String, PairingCodes>>,
}

impl PairingRegistry {
    pub fn issue(&self, instance: &str, clock: &dyn Clock) -> Result<String, String> {
        self.with(instance, |codes| codes.issue(clock))
    }

    pub fn attempt(&self, instance: &str, code: &str, now: i64) -> PairAttempt {
        self.with(instance, |codes| codes.attempt(code, now))
    }

    /// Dimentica i codici di un'istanza (fermata, eliminata).
    pub fn forget(&self, instance: &str) {
        self.instances.lock().unwrap_or_else(PoisonError::into_inner).remove(instance);
    }

    fn with<R>(&self, instance: &str, f: impl FnOnce(&mut PairingCodes) -> R) -> R {
        f(self.instances.lock().unwrap_or_else(PoisonError::into_inner).entry(instance.to_string()).or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::testing::FakeClock;

    const MIN: i64 = 60_000;

    #[test]
    fn a_code_has_12_unambiguous_characters_and_is_consumed_once() {
        let clock = FakeClock::new(1_000_000);
        let mut codes = PairingCodes::default();
        let code = codes.issue(&clock).unwrap();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        assert!(!code.contains(['0', 'O', '1', 'I', 'L']));
        assert_eq!(codes.attempt(&code.to_lowercase(), clock.now_ms()), PairAttempt::Accepted);
        assert_eq!(codes.attempt(&code, clock.now_ms()), PairAttempt::Rejected, "monouso");
    }

    #[test]
    fn two_codes_differ() {
        let clock = FakeClock::new(0);
        let mut codes = PairingCodes::default();
        assert_ne!(codes.issue(&clock).unwrap(), codes.issue(&clock).unwrap());
    }

    #[test]
    fn a_code_expires_after_ten_minutes() {
        let clock = FakeClock::new(0);
        let mut codes = PairingCodes::default();
        let code = codes.issue(&clock).unwrap();
        assert_eq!(codes.attempt(&code, 10 * MIN), PairAttempt::Rejected);
        let fresh = codes.issue(&clock).unwrap();
        assert_eq!(codes.attempt(&fresh, 10 * MIN - 1), PairAttempt::Accepted);
    }

    #[test]
    fn a_wrong_code_is_rejected() {
        let clock = FakeClock::new(0);
        let mut codes = PairingCodes::default();
        codes.issue(&clock).unwrap();
        assert_eq!(codes.attempt("AAAAAAAAAAAA", 1), PairAttempt::Rejected);
        assert_eq!(codes.attempt("", 2), PairAttempt::Rejected);
    }

    #[test]
    fn after_five_attempts_the_pending_codes_are_cancelled_and_even_the_right_one_fails() {
        let clock = FakeClock::new(0);
        let mut codes = PairingCodes::default();
        let code = codes.issue(&clock).unwrap();
        for i in 0..ATTEMPTS_MAX {
            assert_eq!(codes.attempt("BBBBBBBBBBBB", i as i64), PairAttempt::Rejected);
        }
        assert_eq!(codes.pending_count(), 0);
        assert_eq!(codes.attempt(&code, 10), PairAttempt::TooManyAttempts);
        // Passata la finestra si puo' riprovare con un codice nuovo.
        let later = 10 * MIN + 100;
        let clock = FakeClock::new(later);
        let fresh = codes.issue(&clock).unwrap();
        assert_eq!(codes.attempt(&fresh, later), PairAttempt::Accepted);
    }

    #[test]
    fn no_more_than_five_codes_wait_and_the_oldest_leaves_first() {
        let clock = FakeClock::new(0);
        let mut codes = PairingCodes::default();
        let first = codes.issue(&clock).unwrap();
        for _ in 0..PENDING_MAX {
            codes.issue(&clock).unwrap();
        }
        assert_eq!(codes.pending_count(), PENDING_MAX);
        assert_eq!(codes.attempt(&first, 1), PairAttempt::Rejected);
    }

    #[test]
    fn the_registry_keeps_instances_apart() {
        let clock = FakeClock::new(0);
        let registry = PairingRegistry::default();
        let code = registry.issue("con-1-1", &clock).unwrap();
        assert_eq!(registry.attempt("con-1-2", &code, 1), PairAttempt::Rejected);
        assert_eq!(registry.attempt("con-1-1", &code, 1), PairAttempt::Accepted);
        let again = registry.issue("con-1-1", &clock).unwrap();
        registry.forget("con-1-1");
        assert_eq!(registry.attempt("con-1-1", &again, 2), PairAttempt::Rejected);
    }
}
