//! Tentativi di `whoami` dopo un handshake senza ruolo: intervallo crescente,
//! durata limitata. Orologio e attesa sono iniettabili, cosi' i test non dormono.

use std::time::{Duration, Instant};

use super::state::Server;
use super::Backend;

/// Tempo e attesa visti dai tentativi.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep(&self, d: Duration);
}

/// L'orologio di sistema.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Ritmo dei tentativi: si parte da `first`, si raddoppia fino a `max`, si
/// smette dopo `budget`.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub first: Duration,
    pub max: Duration,
    pub budget: Duration,
}

/// Ritmo del server reale: l'app ha cinque minuti per farsi viva.
pub const RETRY_POLICY: RetryPolicy =
    RetryPolicy { first: Duration::from_secs(1), max: Duration::from_secs(10), budget: Duration::from_secs(300) };

/// Riprova a risolvere il ruolo finche' riesce (anche per mano del ciclo
/// principale) o finche' dura il budget. True se il ruolo e' risolto.
pub fn retry_role(server: &Server, backend: &dyn Backend, clock: &dyn Clock, policy: &RetryPolicy) -> bool {
    let deadline = clock.now() + policy.budget;
    let mut delay = policy.first;
    loop {
        let now = clock.now();
        if now >= deadline {
            return false;
        }
        clock.sleep(delay.min(deadline - now));
        if server.role(backend).is_ok() {
            return true;
        }
        delay = delay.saturating_mul(2).min(policy.max);
    }
}
