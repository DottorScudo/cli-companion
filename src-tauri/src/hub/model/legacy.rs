//! Lettura dei campi dei thread scritti in una forma precedente.

use serde::{Deserialize, Deserializer};

/// Le proposte collegate a un thread: l'elenco `proposal_ids` o, nei file
/// scritti prima, il vecchio `proposal_id` (una stringa o `null`).
pub fn proposal_ids<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Many(Vec<String>),
        One(Option<String>),
    }
    Ok(match Stored::deserialize(deserializer)? {
        Stored::Many(ids) => ids,
        Stored::One(id) => id.into_iter().collect(),
    })
}
