//! Il cursore del connettore, `{"offset": n}` (puro): il prossimo `update_id` da chiedere.

use serde_json::{json, Value};

/// L'offset salvato. Un cursore assente o malformato e' "nessun cursore" (si riparte da adesso).
pub fn parse(cursor: Option<&Value>) -> Option<i64> {
    cursor?.get("offset")?.as_i64()
}

pub fn to_value(offset: i64) -> Value {
    json!({ "offset": offset })
}

#[cfg(test)]
mod tests;
