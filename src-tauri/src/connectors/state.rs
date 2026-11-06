//! Lo stato di un'istanza (`connector_state/<istanza>.json`): cursori, id gia'
//! visti per la dedupe, mappe delle conversazioni e dei messaggi, e le chiavi
//! private del connettore. Puro e limitato: l'insieme dei visti tiene al piu'
//! `SEEN_MAX` id (i piu' vecchi escono per primi), ciascuna mappa `MAP_MAX` voci
//! (le meno recenti escono per prime). Mai segreti qui: il file e' in chiaro.

use super::limits::{CUSTOM_KEYS_MAX, CUSTOM_VALUE_MAX, MAP_MAX, SEEN_MAX};
use super::runtime::contact::subchats::SubchatMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Chiave del cursore di un connettore che ne ha uno solo (`contact`, o una
/// fonte che non lo tiene per regola).
pub const MAIN_CURSOR: &str = "main";

/// Insieme di stringhe con un tetto: oltre `cap` esce la piu' vecchia. Si salva
/// come elenco, dalla piu' vecchia alla piu' recente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<String>", into = "Vec<String>")]
pub struct BoundedSet {
    cap: usize,
    order: VecDeque<String>,
    members: HashSet<String>,
}

impl BoundedSet {
    pub fn with_capacity(cap: usize) -> Self {
        Self { cap, order: VecDeque::new(), members: HashSet::new() }
    }

    pub fn contains(&self, value: &str) -> bool {
        self.members.contains(value)
    }

    /// Aggiunge `value` (se c'e' gia' non cambia nulla, nemmeno l'ordine). Ritorna
    /// `true` se era nuovo.
    pub fn insert(&mut self, value: &str) -> bool {
        if !self.members.insert(value.to_string()) {
            return false;
        }
        self.order.push_back(value.to_string());
        while self.order.len() > self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.members.remove(&oldest);
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }
}

impl Default for BoundedSet {
    fn default() -> Self {
        Self::with_capacity(SEEN_MAX)
    }
}

impl From<Vec<String>> for BoundedSet {
    /// Da un file: i duplicati si ignorano e oltre il tetto restano i piu' recenti.
    fn from(list: Vec<String>) -> Self {
        let mut set = Self::default();
        for value in &list {
            set.insert(value);
        }
        set
    }
}

impl From<BoundedSet> for Vec<String> {
    fn from(set: BoundedSet) -> Self {
        set.order.into()
    }
}

/// Mappa di stringhe con un tetto: oltre `cap` esce la voce meno recente (per
/// inserimento o per accesso con `touch`). Si salva come elenco di coppie, dalla
/// meno recente alla piu' recente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<(String, String)>", into = "Vec<(String, String)>")]
pub struct BoundedMap {
    cap: usize,
    order: VecDeque<String>,
    entries: HashMap<String, String>,
}

impl BoundedMap {
    pub fn with_capacity(cap: usize) -> Self {
        Self { cap, order: VecDeque::new(), entries: HashMap::new() }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Come `get`, ma la voce diventa la piu' recente.
    pub fn touch(&mut self, key: &str) -> Option<&str> {
        if self.entries.contains_key(key) {
            self.move_to_newest(key);
        }
        self.get(key)
    }

    /// Inserisce o sostituisce: la voce diventa la piu' recente.
    pub fn insert(&mut self, key: &str, value: &str) {
        if self.entries.insert(key.to_string(), value.to_string()).is_some() {
            self.move_to_newest(key);
        } else {
            self.order.push_back(key.to_string());
        }
        while self.order.len() > self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        let value = self.entries.remove(key)?;
        self.order.retain(|k| k != key);
        Some(value)
    }

    /// Tiene solo le voci il cui valore soddisfa `keep` (es. i thread ancora esistenti).
    pub fn retain_values(&mut self, mut keep: impl FnMut(&str) -> bool) {
        let entries = &mut self.entries;
        entries.retain(|_, v| keep(v));
        self.order.retain(|k| entries.contains_key(k));
    }

    /// Tiene solo le voci la cui chiave soddisfa `keep`.
    pub fn retain_keys(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.entries.retain(|k, _| keep(k));
        let entries = &self.entries;
        self.order.retain(|k| entries.contains_key(k));
    }

    /// Toglie le voci la cui chiave inizia con `prefix`.
    pub fn remove_prefix(&mut self, prefix: &str) {
        self.entries.retain(|k, _| !k.starts_with(prefix));
        let entries = &self.entries;
        self.order.retain(|k| entries.contains_key(k));
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    fn move_to_newest(&mut self, key: &str) {
        self.order.retain(|k| k != key);
        self.order.push_back(key.to_string());
    }
}

impl Default for BoundedMap {
    fn default() -> Self {
        Self::with_capacity(MAP_MAX)
    }
}

impl From<Vec<(String, String)>> for BoundedMap {
    fn from(list: Vec<(String, String)>) -> Self {
        let mut map = Self::default();
        for (key, value) in &list {
            map.insert(key, value);
        }
        map
    }
}

impl From<BoundedMap> for Vec<(String, String)> {
    fn from(map: BoundedMap) -> Self {
        let BoundedMap { order, mut entries, .. } = map;
        order.into_iter().filter_map(|key| entries.remove(&key).map(|value| (key, value))).collect()
    }
}

/// Chiave dei visti per una regola: gli id delle regole non contengono `:`.
pub fn seen_key(route: &str, external_id: &str) -> String {
    format!("{route}:{external_id}")
}

/// Chiave delle conversazioni per una regola.
pub fn conversation_key(route: &str, key: &str) -> String {
    format!("{route}:{key}")
}

/// Perche' una chiave privata del connettore non si puo' salvare.
const CUSTOM_KEY_INVALID: &str = "chiave dello stato non valida";

/// Lo stato persistente di un'istanza.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceState {
    /// Cursori per chiave (`MAIN_CURSOR` o l'id di una regola). Assente = si parte da "adesso".
    #[serde(default)]
    pub cursors: BTreeMap<String, Value>,
    /// `(regola, id esterno)` gia' consegnati.
    #[serde(default)]
    pub seen: BoundedSet,
    /// `(regola, chiave di conversazione)` -> id del thread (`per_conversation`).
    #[serde(default)]
    pub conversations: BoundedMap,
    /// Id esterno di un messaggio -> id del messaggio dell'HUB.
    #[serde(default)]
    pub external_to_hub: BoundedMap,
    /// Id del messaggio dell'HUB -> id esterno.
    #[serde(default)]
    pub hub_to_external: BoundedMap,
    /// Thread dell'HUB <-> sotto-chat delle chat collegate (canali di contatto,
    /// `runtime::contact::subchats`). Assente in uno stato vecchio = vuota.
    #[serde(default)]
    pub subchats: SubchatMap,
    /// Chiavi private del connettore (`[a-z0-9_]`, poche, di dimensione limitata).
    #[serde(default)]
    pub custom: BTreeMap<String, Value>,
}

impl InstanceState {
    pub fn cursor(&self, key: &str) -> Option<&Value> {
        self.cursors.get(key)
    }

    pub fn set_cursor(&mut self, key: &str, cursor: Value) {
        self.cursors.insert(key.to_string(), cursor);
    }

    /// L'elemento e' gia' stato consegnato per questa regola?
    pub fn is_seen(&self, route: &str, external_id: &str) -> bool {
        self.seen.contains(&seen_key(route, external_id))
    }

    pub fn mark_seen(&mut self, route: &str, external_id: &str) {
        self.seen.insert(&seen_key(route, external_id));
    }

    /// Lega un messaggio esterno a uno dell'HUB, nei due versi. `stored` e' cio' che
    /// l'id esterno ritrova (il riferimento completo al messaggio, `contact::refs`).
    pub fn link_message_ref(&mut self, external_id: &str, hub_message_id: &str, stored: &str) {
        self.external_to_hub.insert(external_id, stored);
        self.hub_to_external.insert(hub_message_id, external_id);
    }

    /// Come `link_message_ref`, con l'id del messaggio dell'HUB come valore.
    #[cfg(test)]
    pub fn link_messages(&mut self, external_id: &str, hub_message_id: &str) {
        self.link_message_ref(external_id, hub_message_id, hub_message_id);
    }

    /// Toglie dai dati dell'istanza tutto cio' che riguarda la regola `route`
    /// (cursore, conversazioni): chiamata quando la regola viene eliminata. I
    /// visti restano: ricreata la regola, non si ripesca l'arretrato.
    #[cfg(test)]
    pub fn forget_route(&mut self, route: &str) {
        self.cursors.remove(route);
        self.conversations.remove_prefix(&format!("{route}:"));
    }

    /// Toglie cursori e conversazioni delle regole che non ci sono piu' (`live`):
    /// il cursore `MAIN_CURSOR` resta. Le chiavi delle conversazioni sono `<regola>:<chiave>`.
    pub fn prune_routes(&mut self, live: &std::collections::HashSet<String>) {
        self.cursors.retain(|k, _| k == MAIN_CURSOR || live.contains(k));
        self.conversations.retain_keys(|k| k.split_once(':').is_some_and(|(route, _)| live.contains(route)));
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "lo legge solo `state_get` del contesto, che nessun connettore registrato usa ancora"
        )
    )]
    pub fn custom_get(&self, key: &str) -> Option<&Value> {
        self.custom.get(key)
    }

    /// Salva un valore privato del connettore, rispettando i tetti.
    pub fn custom_set(&mut self, key: &str, value: Value) -> Result<(), String> {
        if !super::descriptor::validate::valid_key(key) {
            return Err(CUSTOM_KEY_INVALID.to_string());
        }
        if !self.custom.contains_key(key) && self.custom.len() >= CUSTOM_KEYS_MAX {
            return Err(format!("troppe chiavi nello stato (al massimo {CUSTOM_KEYS_MAX})"));
        }
        if value.to_string().len() > CUSTOM_VALUE_MAX {
            return Err(format!("valore dello stato oltre {CUSTOM_VALUE_MAX} byte"));
        }
        self.custom.insert(key.to_string(), value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_seen_set_holds_at_most_5000_and_drops_the_oldest() {
        let mut state = InstanceState::default();
        for i in 0..SEEN_MAX + 10 {
            state.mark_seen("rte-1-1", &format!("m{i}"));
        }
        assert_eq!(state.seen.len(), SEEN_MAX);
        assert!(!state.is_seen("rte-1-1", "m0") && !state.is_seen("rte-1-1", "m9"));
        assert!(state.is_seen("rte-1-1", "m10") && state.is_seen("rte-1-1", &format!("m{}", SEEN_MAX + 9)));
        // La dedupe e' per regola.
        assert!(!state.is_seen("rte-1-2", "m10"));
    }

    #[test]
    fn inserting_a_seen_id_twice_neither_grows_nor_reorders() {
        let mut set = BoundedSet::with_capacity(3);
        assert!(set.insert("a") && set.insert("b") && !set.insert("a"));
        set.insert("c");
        set.insert("d");
        assert!(!set.contains("a"), "a e' la piu' vecchia e non e' stata rinfrescata");
        assert_eq!(Vec::from(set), vec!["b", "c", "d"]);
    }

    #[test]
    fn maps_hold_at_most_2000_and_drop_the_least_recent() {
        let mut state = InstanceState::default();
        for i in 0..MAP_MAX + 5 {
            state.conversations.insert(&format!("k{i}"), "thr-1");
            state.link_messages(&format!("e{i}"), &format!("h{i}"));
        }
        assert_eq!(state.conversations.len(), MAP_MAX);
        assert_eq!(state.external_to_hub.len(), MAP_MAX);
        assert_eq!(state.hub_to_external.len(), MAP_MAX);
        assert_eq!(state.conversations.get("k0"), None);
        assert_eq!(state.conversations.get("k5"), Some("thr-1"));
        assert_eq!(state.external_to_hub.get("e2004"), Some("h2004"));
        assert_eq!(state.hub_to_external.get("h2004"), Some("e2004"));
    }

    #[test]
    fn touching_or_replacing_an_entry_makes_it_the_newest() {
        let mut map = BoundedMap::with_capacity(3);
        map.insert("a", "1");
        map.insert("b", "2");
        map.insert("c", "3");
        assert_eq!(map.touch("a"), Some("1"));
        map.insert("d", "4");
        assert_eq!(map.get("b"), None, "b era la meno recente dopo il touch di a");
        map.insert("c", "30");
        map.insert("e", "5");
        assert_eq!((map.get("a"), map.get("c"), map.get("d")), (None, Some("30"), Some("4")));
        assert_eq!(map.touch("zzz"), None);
    }

    #[test]
    fn retain_values_and_remove_keep_the_order_consistent() {
        let mut map = BoundedMap::with_capacity(5);
        for (k, v) in [("a", "x"), ("b", "y"), ("c", "x")] {
            map.insert(k, v);
        }
        map.retain_values(|v| v != "x");
        assert_eq!((map.len(), map.get("b")), (1, Some("y")));
        assert_eq!(map.remove("b").as_deref(), Some("y"));
        assert_eq!((map.len(), map.remove("b")), (0, None));
    }

    #[test]
    fn the_state_roundtrips_through_json_keeping_the_order() {
        let mut state = InstanceState::default();
        state.set_cursor(MAIN_CURSOR, json!({"offset": 7}));
        state.mark_seen("rte-1-1", "a");
        state.mark_seen("rte-1-1", "b");
        state.conversations.insert("rte-1-1:t1", "thr-1-1");
        state.link_messages("e1", "h1");
        state.custom_set("bot_name", json!("mio")).unwrap();
        let text = serde_json::to_string(&state).unwrap();
        let back: InstanceState = serde_json::from_str(&text).unwrap();
        assert_eq!(back, state);
        assert_eq!(back.cursor(MAIN_CURSOR), Some(&json!({"offset": 7})));
        assert_eq!(Vec::from(back.seen), vec!["rte-1-1:a", "rte-1-1:b"]);
    }

    #[test]
    fn an_oversized_file_is_trimmed_on_load_keeping_the_newest() {
        let ids: Vec<String> = (0..SEEN_MAX + 3).map(|i| format!("r:{i}")).collect();
        let text = json!({ "seen": ids, "external_to_hub": (0..MAP_MAX + 3).map(|i| [format!("e{i}"), "h".to_string()]).collect::<Vec<_>>() });
        let state: InstanceState = serde_json::from_value(text).unwrap();
        assert_eq!(state.seen.len(), SEEN_MAX);
        assert!(!state.seen.contains("r:2") && state.seen.contains("r:3"));
        assert_eq!(state.external_to_hub.len(), MAP_MAX);
        assert_eq!(state.external_to_hub.get("e2"), None);
    }

    #[test]
    fn an_old_state_without_the_subchat_map_reads_it_empty_and_the_map_roundtrips() {
        use crate::connectors::runtime::contact::subchats::{SubchatKey, ThreadKey};
        use crate::hub::Channel;
        let old = json!({"cursors": {"main": 7}, "external_to_hub": [["e1", "general thr-1 msg-1"]]});
        let mut state: InstanceState = serde_json::from_value(old).unwrap();
        assert_eq!(state.subchats.len(), 0);
        assert_eq!(state.external_to_hub.get("e1"), Some("general thr-1 msg-1"));
        let thread = ThreadKey { channel: Channel::General, thread_id: "thr-1".to_string() };
        assert!(state.subchats.insert(thread.clone(), SubchatKey { chat: "g1".into(), subchat: "7".into() }));
        let mut back: InstanceState = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(back.subchats.subchat_of(&thread, "g1").as_deref(), Some("7"));
    }

    #[test]
    fn an_empty_or_partial_file_gets_defaults() {
        let state: InstanceState = serde_json::from_str("{}").unwrap();
        assert_eq!(state, InstanceState::default());
        assert!(state.cursors.is_empty() && state.cursor(MAIN_CURSOR).is_none());
    }

    #[test]
    fn forgetting_a_route_drops_its_cursor_and_conversations_only() {
        let mut state = InstanceState::default();
        state.set_cursor("rte-1-1", json!(1));
        state.set_cursor("rte-1-2", json!(2));
        state.conversations.insert("rte-1-1:t", "thr-1");
        state.conversations.insert("rte-1-2:t", "thr-2");
        state.mark_seen("rte-1-1", "x");
        state.forget_route("rte-1-1");
        assert!(state.cursor("rte-1-1").is_none() && state.cursor("rte-1-2").is_some());
        assert_eq!((state.conversations.get("rte-1-1:t"), state.conversations.get("rte-1-2:t")), (None, Some("thr-2")));
        assert!(state.is_seen("rte-1-1", "x"));
    }

    #[test]
    fn custom_keys_are_validated_and_bounded() {
        let mut state = InstanceState::default();
        assert!(state.custom_set("Bad Key", json!(1)).is_err());
        assert!(state.custom_set("", json!(1)).is_err());
        for i in 0..CUSTOM_KEYS_MAX {
            state.custom_set(&format!("k{i}"), json!(i)).unwrap();
        }
        assert!(state.custom_set("one_more", json!(1)).unwrap_err().contains("troppe"));
        // Una chiave esistente si puo' sempre aggiornare.
        state.custom_set("k0", json!("nuovo")).unwrap();
        assert_eq!(state.custom_get("k0"), Some(&json!("nuovo")));
        let huge = json!("x".repeat(CUSTOM_VALUE_MAX));
        assert!(state.custom_set("k1", huge).unwrap_err().contains("oltre"));
    }
}
