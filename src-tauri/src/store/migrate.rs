//! Catena di migrazioni: passi puri, uno per salto di versione.

use serde_json::Value;

/// Passo N -> N+1: funzione pura sul JSON del dato (busta esclusa).
pub type Migration = fn(Value) -> Result<Value, String>;

/// Passo 0 -> 1: introduce la busta `{schema_version, data}`, il dato non cambia.
pub fn adopt_envelope(v: Value) -> Result<Value, String> {
    Ok(v)
}

/// Applica in ordine `steps[from_version..]`. La versione corrente di un
/// file e' `steps.len()`. L'errore nomina il passo che ha fallito.
pub(crate) fn run_chain(value: Value, from_version: u32, steps: &[Migration]) -> Result<Value, String> {
    let mut value = value;
    for (index, step) in steps.iter().enumerate().skip(from_version as usize) {
        value = step(value).map_err(|e| format!("migrazione {index} -> {}: {e}", index + 1))?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rename_a_to_b(mut v: Value) -> Result<Value, String> {
        let obj = v.as_object_mut().ok_or("non e' un oggetto")?;
        if let Some(a) = obj.remove("a") {
            obj.insert("b".into(), a);
        }
        Ok(v)
    }

    fn add_c(mut v: Value) -> Result<Value, String> {
        let obj = v.as_object_mut().ok_or("non e' un oggetto")?;
        obj.entry("c").or_insert(json!(7));
        Ok(v)
    }

    #[test]
    fn adopt_envelope_is_identity() {
        assert_eq!(adopt_envelope(json!({"x": 1})).unwrap(), json!({"x": 1}));
    }

    #[test]
    fn steps_run_in_order_from_the_file_version() {
        let steps: [Migration; 2] = [rename_a_to_b, add_c];
        assert_eq!(run_chain(json!({"a": 1}), 0, &steps).unwrap(), json!({"b": 1, "c": 7}));
        // Versione 1: il primo passo e' gia' applicato, resta solo il secondo.
        assert_eq!(run_chain(json!({"b": 1}), 1, &steps).unwrap(), json!({"b": 1, "c": 7}));
        // Versione corrente: nessun passo.
        assert_eq!(run_chain(json!({"a": 1}), 2, &steps).unwrap(), json!({"a": 1}));
    }

    #[test]
    fn failing_step_is_named() {
        let steps: [Migration; 2] = [adopt_envelope, rename_a_to_b];
        let err = run_chain(json!([1, 2]), 0, &steps).unwrap_err();
        assert!(err.contains("1 -> 2"), "{err}");
    }
}
