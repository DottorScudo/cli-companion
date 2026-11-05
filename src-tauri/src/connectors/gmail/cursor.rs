//! Il cursore del connettore, per regola, in un solo valore JSON (puro):
//!
//! ```json
//! { "routes": { "<id regola>": { "floor_ms": 0, "last_ms": 0, "recent": [["<id messaggio>", 0]] } } }
//! ```
//!
//! Il runtime lo salva solo dopo la consegna: un giro interrotto rilegge, e la dedupe
//! per id esterno evita i doppioni.

use crate::connectors::limits::MAX_ROUTES_PER_INSTANCE;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Margine all'indietro della ricerca: l'indice di ricerca di Gmail puo' ritardare
/// di qualche minuto, e un messaggio arrivato «prima» di `last_ms` potrebbe comparire dopo.
pub const MARGIN_MS: i64 = 10 * 60 * 1000;
/// Id ricordati al massimo per regola: i piu' vecchi escono per primi.
pub const RECENT_MAX: usize = 200;
/// Millisecondi in un secondo.
const MS_PER_SEC: i64 = 1000;
/// Lunghezza massima dell'id di una regola nel cursore.
const ROUTE_ID_MAX: usize = 256;

/// Il cursore di una regola.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCursor {
    /// Quando e' nata la regola: nessuna email piu' vecchia si importa mai.
    floor_ms: i64,
    /// Il `internalDate` piu' recente visto: da qui riparte la ricerca (meno il margine).
    last_ms: i64,
    /// I messaggi gia' visti nella finestra del margine (id, `internalDate`): la ricerca li
    /// restituisce ancora, e senza questa memoria si consegnerebbero due volte.
    recent: Vec<(String, i64)>,
}

impl RouteCursor {
    fn starting_at(now_ms: i64) -> Self {
        Self { floor_ms: now_ms, last_ms: now_ms, recent: Vec::new() }
    }

    /// Il parametro `after:` della ricerca, in secondi: mai prima della nascita della regola.
    pub fn after_secs(&self) -> i64 {
        self.last_ms.saturating_sub(MARGIN_MS).max(self.floor_ms).div_euclid(MS_PER_SEC)
    }

    /// Il messaggio e' gia' stato visto di recente?
    pub fn is_recent(&self, id: &str) -> bool {
        self.recent.iter().any(|(seen, _)| seen == id)
    }

    /// Un messaggio piu' vecchio della regola non si importa: la ricerca `after:` ha la
    /// granularita' del secondo e il margine puo' riportare indietro messaggi precedenti.
    pub fn accepts(&self, internal_ms: i64) -> bool {
        internal_ms >= self.floor_ms
    }

    /// Ricorda un messaggio gia' consegnato (o scartato), senza doppioni.
    pub fn remember(&mut self, id: &str, internal_ms: i64) {
        if !self.is_recent(id) {
            self.recent.push((id.to_string(), internal_ms));
        }
        self.prune();
    }

    /// Porta avanti il punto di ripartenza (mai indietro).
    pub fn advance(&mut self, internal_ms: i64) {
        self.last_ms = self.last_ms.max(internal_ms);
        self.prune();
    }

    /// Fuori gli id che la ricerca non restituira' piu' (oltre il margine) e, se troppi, i piu' vecchi.
    fn prune(&mut self) {
        let horizon = self.last_ms.saturating_sub(MARGIN_MS);
        self.recent.retain(|(_, ms)| *ms >= horizon);
        if self.recent.len() > RECENT_MAX {
            self.recent.sort_by_key(|(_, ms)| *ms);
            let excess = self.recent.len() - RECENT_MAX;
            self.recent.drain(..excess);
        }
    }

    fn to_value(&self) -> Value {
        json!({"floor_ms": self.floor_ms, "last_ms": self.last_ms, "recent": self.recent})
    }

    /// Una voce illeggibile si scarta (la regola ripartira' da adesso); gli id illeggibili si saltano.
    fn parse(value: &Value) -> Option<Self> {
        let floor_ms = value.get("floor_ms")?.as_i64()?;
        let last_ms = value.get("last_ms")?.as_i64()?;
        let recent = value
            .get("recent")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(recent_entry).take(RECENT_MAX).collect())
            .unwrap_or_default();
        let mut route = Self { floor_ms, last_ms, recent };
        route.prune();
        Some(route)
    }
}

/// `["<id>", <internal_ms>]`.
fn recent_entry(value: &Value) -> Option<(String, i64)> {
    let pair = value.as_array()?;
    let id = pair.first()?.as_str().filter(|id| !id.is_empty() && id.len() <= ROUTE_ID_MAX)?;
    Some((id.to_string(), pair.get(1)?.as_i64()?))
}

/// Il cursore dell'istanza: una voce per regola.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cursor {
    routes: BTreeMap<String, RouteCursor>,
}

impl Cursor {
    /// Tollerante: un cursore assente o rovinato non e' un errore; le regole senza voce
    /// valida ripartono da adesso (`route_mut`).
    pub fn parse(value: Option<&Value>) -> Self {
        let routes = value
            .and_then(|v| v.get("routes"))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter(|(id, _)| !id.is_empty() && id.len() <= ROUTE_ID_MAX)
                    .filter_map(|(id, v)| Some((id.clone(), RouteCursor::parse(v)?)))
                    .take(MAX_ROUTES_PER_INSTANCE)
                    .collect()
            })
            .unwrap_or_default();
        Self { routes }
    }

    pub fn to_value(&self) -> Value {
        let routes: Map<String, Value> = self.routes.iter().map(|(id, r)| (id.clone(), r.to_value())).collect();
        json!({ "routes": routes })
    }

    /// La regola ha gia' una voce? Una regola senza voce e' appena nata.
    pub fn has_route(&self, route_id: &str) -> bool {
        self.routes.contains_key(route_id)
    }

    /// Il cursore della regola; per una regola nuova nasce adesso, cosi' non si importa
    /// mai un'email piu' vecchia della regola.
    pub fn route_mut(&mut self, route_id: &str, now_ms: i64) -> &mut RouteCursor {
        self.routes.entry(route_id.to_string()).or_insert_with(|| RouteCursor::starting_at(now_ms))
    }

    /// Tiene solo le regole attive: quelle cancellate non lasciano residui.
    pub fn retain(&mut self, live_route_ids: &[String]) {
        self.routes.retain(|id, _| live_route_ids.contains(id));
    }
}

#[cfg(test)]
mod tests;
