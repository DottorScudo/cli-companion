//! Cache in-process con scadenza.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cache in-process con scadenza: i risultati di operazioni lente (processi
/// esterni, rilevamento WSL) non vanno rifatti a ogni richiesta.
pub(crate) struct TtlCache<K, V> {
    ttl: Duration,
    map: Mutex<HashMap<K, (Instant, V)>>,
}

impl<K: Eq + Hash, V: Clone> TtlCache<K, V> {
    pub(crate) fn new(ttl: Duration) -> Self {
        Self { ttl, map: Mutex::new(HashMap::new()) }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        let map = self.map.lock().unwrap();
        map.get(key).filter(|(at, _)| at.elapsed() < self.ttl).map(|(_, v)| v.clone())
    }

    pub(crate) fn put(&self, key: K, value: V) {
        self.map.lock().unwrap().insert(key, (Instant::now(), value));
    }

    pub(crate) fn clear(&self) {
        self.map.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_cache_expires_and_clears() {
        let c = TtlCache::new(Duration::from_millis(50));
        c.put(1, Some("a".to_string()));
        assert_eq!(c.get(&1), Some(Some("a".to_string())));
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(c.get(&1), None);
        c.put(2, None);
        c.clear();
        assert_eq!(c.get(&2), None);
    }
}
