//! Gli id generati dai connettori: `con-<ms>-<n>` (istanza), `rte-<ms>-<n>`
//! (regola), `lnk-<ms>-<n>` (collegamento). L'id di un'istanza finisce nel nome
//! del file di stato e nell'account del portachiavi: la validazione e' stretta.

use crate::hub::valid_ref_id;
use std::sync::atomic::{AtomicU64, Ordering};

pub const INSTANCE_PREFIX: &str = "con";
pub const ROUTE_PREFIX: &str = "rte";
pub const LINK_PREFIX: &str = "lnk";

/// Genera gli id: il contatore rende unici quelli dello stesso millisecondo.
/// Stato gestito dal runtime; l'orologio lo passa il chiamante.
#[derive(Default)]
pub struct IdGenerator {
    sequence: AtomicU64,
}

impl IdGenerator {
    /// Nuovo id `<prefisso>-<ms>-<n>`.
    pub fn next(&self, prefix: &str, now_ms: i64) -> String {
        let n = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{now_ms}-{n}")
    }
}

/// `<prefisso>-<cifre>-<cifre>` con `valid_ref_id` (lunghezza e caratteri ammessi).
fn valid_with_prefix(id: &str, prefix: &str) -> bool {
    if !valid_ref_id(id) {
        return false;
    }
    let mut parts = id.split('-');
    let digits = |part: Option<&str>| part.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    parts.next() == Some(prefix) && digits(parts.next()) && digits(parts.next()) && parts.next().is_none()
}

pub fn valid_instance_id(id: &str) -> bool {
    valid_with_prefix(id, INSTANCE_PREFIX)
}

pub fn valid_route_id(id: &str) -> bool {
    valid_with_prefix(id, ROUTE_PREFIX)
}

pub fn valid_link_id(id: &str) -> bool {
    valid_with_prefix(id, LINK_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_valid_and_unique_in_the_same_millisecond() {
        let ids = IdGenerator::default();
        let a = ids.next(INSTANCE_PREFIX, 1_700_000_000_000);
        let b = ids.next(INSTANCE_PREFIX, 1_700_000_000_000);
        assert_eq!(a, "con-1700000000000-1");
        assert_ne!(a, b);
        assert!(valid_instance_id(&a) && valid_instance_id(&b));
        assert!(valid_route_id(&ids.next(ROUTE_PREFIX, 5)));
        assert!(valid_link_id(&ids.next(LINK_PREFIX, 5)));
    }

    #[test]
    fn each_validator_accepts_only_its_own_prefix_and_shape() {
        assert!(!valid_instance_id("rte-1-1"));
        assert!(!valid_route_id("con-1-1"));
        assert!(!valid_link_id("con-1-1"));
        for bad in
            ["", "con", "con-1", "con--1", "con-1-", "con-a-1", "con-1-1-1", "con-1-1/..", "con-1-1 ", "../con-1-1"]
        {
            assert!(!valid_instance_id(bad), "{bad:?}");
        }
        assert!(!valid_instance_id(&format!("con-1-{}", "9".repeat(80))));
        assert!(!valid_instance_id("con-１-1"), "cifre non ASCII");
    }
}
