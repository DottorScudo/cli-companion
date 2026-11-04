//! Formato su disco: busta `{ "schema_version": N, "data": ... }`.

use serde::Serialize;
use serde_json::{Map, Value};

const KEY_VERSION: &str = "schema_version";
const KEY_DATA: &str = "data";

#[derive(Serialize)]
struct EnvelopeRef<'a, T> {
    schema_version: u32,
    data: &'a T,
}

/// Serializza il dato dentro la busta (JSON indentato).
pub(crate) fn to_pretty<T: Serialize>(schema_version: u32, data: &T) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&EnvelopeRef { schema_version, data })
}

/// Versione dichiarata da un oggetto che ha ESATTAMENTE le chiavi della busta.
fn envelope_version(map: &Map<String, Value>) -> Option<u32> {
    if map.len() != 2 || !map.contains_key(KEY_DATA) {
        return None;
    }
    u32::try_from(map.get(KEY_VERSION)?.as_u64()?).ok()
}

/// Separa versione e dato. Qualunque JSON che non sia una busta e' il formato
/// storico: versione 0 con l'intero valore come dato.
pub(crate) fn split(value: Value) -> (u32, Value) {
    match value {
        Value::Object(mut map) => match envelope_version(&map) {
            Some(version) => (version, map.remove(KEY_DATA).unwrap_or(Value::Null)),
            None => (0, Value::Object(map)),
        },
        other => (0, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_version_and_data() {
        let raw = to_pretty(3, &json!({"a": 1})).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v, json!({"schema_version": 3, "data": {"a": 1}}));
        assert!(raw.contains('\n'), "deve essere indentato");
    }

    #[test]
    fn exact_envelope_is_recognized() {
        assert_eq!(split(json!({"schema_version": 2, "data": {"a": 1}})), (2, json!({"a": 1})));
        assert_eq!(split(json!({"schema_version": 0, "data": null})), (0, Value::Null));
    }

    #[test]
    fn anything_else_is_legacy_version_zero() {
        for legacy in [
            json!({}),
            json!({"a": 1}),
            json!([1, 2]),
            json!("testo"),
            json!(null),
            json!({"schema_version": 1}),
            json!({"data": {}}),
            json!({"schema_version": 1, "data": {}, "extra": true}),
            json!({"schema_version": "1", "data": {}}),
            json!({"schema_version": -1, "data": {}}),
            json!({"schema_version": 1.5, "data": {}}),
            json!({"schema_version": 4_294_967_296_u64, "data": {}}),
        ] {
            assert_eq!(split(legacy.clone()), (0, legacy.clone()), "{legacy}");
        }
    }
}
