//! Validazione pura, senza I/O: del descrittore stesso (per i test di conformita')
//! e dei valori di configurazione o di filtro contro i campi dichiarati (tipi,
//! obbligatorieta', limiti, chiavi sconosciute rifiutate, segreti fuori dalla
//! configurazione). La validazione semantica (una query non vuota, un indirizzo
//! plausibile) e' del connettore. I messaggi sono in italiano, leggibili
//! dall'utente, e non riportano mai il valore rifiutato.

use super::{ConfigField, FieldKind};
use crate::connectors::limits;
use crate::hub::{valid_ref_id, Channel};
use serde_json::{Map, Value};

#[cfg(test)]
mod check;
#[cfg(test)]
pub use check::{valid_host, validate_descriptor};

/// Lunghezza massima dell'id di un connettore.
const ID_MAX: usize = 32;
/// Lunghezza massima della chiave di un campo o di uno slot.
pub const KEY_MAX: usize = 40;

/// `[a-z0-9-]`, 1..=32 caratteri, senza trattini ai bordi.
pub fn valid_connector_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_MAX
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `[a-z0-9_]`, 1..=40 caratteri: chiavi dei campi e slot del portachiavi.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= KEY_MAX
        && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Toglie gli spazi ai bordi dei campi `text` (gli altri valori restano come
/// sono). Si applica al valore ricevuto dall'utente prima di `validate_values`.
pub fn normalize_values(fields: &[ConfigField], mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        for field in fields.iter().filter(|f| matches!(f.kind, FieldKind::Text { .. })) {
            if let Some(Value::String(text)) = map.get_mut(field.key) {
                let trimmed = text.trim();
                if trimmed.len() != text.len() {
                    *text = trimmed.to_string();
                }
            }
        }
    }
    value
}

/// Controlla `value` (configurazione o filtro) contro `fields`: deve essere un
/// oggetto di dimensione ragionevole, senza chiavi sconosciute e senza campi
/// `secret` (vanno nel portachiavi), con i tipi e i limiti dichiarati e tutti i
/// campi obbligatori presenti.
pub fn validate_values(fields: &[ConfigField], value: &Value) -> Result<(), String> {
    let Value::Object(map) = value else { return Err("deve essere un oggetto".to_string()) };
    if value.to_string().len() > limits::CONFIG_JSON_MAX {
        return Err(format!("troppo grande (oltre {} byte)", limits::CONFIG_JSON_MAX));
    }
    for key in map.keys() {
        match fields.iter().find(|f| f.key == key) {
            None => return Err(format!("campo sconosciuto: «{}»", shown_key(key))),
            Some(field) if matches!(field.kind, FieldKind::Secret) => {
                return Err(format!("«{}» è un segreto: non va nella configurazione ma nel portachiavi", field.key));
            }
            Some(_) => {}
        }
    }
    fields.iter().filter(|f| !matches!(f.kind, FieldKind::Secret)).try_for_each(|f| validate_field(f, map))
}

/// La chiave sconosciuta, ridotta perche' un messaggio non porti testo arbitrario lungo.
fn shown_key(key: &str) -> String {
    key.chars().filter(|c| !c.is_control()).take(KEY_MAX).collect()
}

fn validate_field(field: &ConfigField, map: &Map<String, Value>) -> Result<(), String> {
    let fail = |why: &str| Err(format!("«{}»: {why}", field.key));
    let value = match map.get(field.key) {
        None | Some(Value::Null) if field.required => return fail("obbligatorio"),
        None | Some(Value::Null) => return Ok(()),
        Some(value) => value,
    };
    match (&field.kind, value) {
        (FieldKind::Text { max_len, .. }, Value::String(s)) => {
            let text = s.trim();
            if text.chars().any(char::is_control) {
                fail("contiene caratteri di controllo")
            } else if field.required && text.is_empty() {
                fail("obbligatorio")
            } else if text.chars().count() > *max_len {
                fail(&format!("oltre {max_len} caratteri"))
            } else {
                Ok(())
            }
        }
        (FieldKind::Text { .. }, _) => fail("deve essere un testo"),
        (FieldKind::Number { min, max }, v) => match v.as_i64() {
            Some(n) if (*min..=*max).contains(&n) => Ok(()),
            Some(_) => fail(&format!("fuori dall'intervallo {min}..={max}")),
            None => fail("deve essere un numero intero"),
        },
        (FieldKind::Bool { .. }, Value::Bool(_)) => Ok(()),
        (FieldKind::Bool { .. }, _) => fail("deve essere vero o falso"),
        (FieldKind::Select { options }, Value::String(s)) if options.iter().any(|o| o.value == s) => Ok(()),
        (FieldKind::Select { .. }, _) => fail("valore non fra le opzioni"),
        (FieldKind::Workspace, Value::String(s)) if matches!(Channel::parse(s), Ok(Channel::Workspace(_))) => Ok(()),
        (FieldKind::Workspace, _) => fail("deve essere l'id di un workspace"),
        (FieldKind::Thread, v) => thread_ref_problem(v).map_or(Ok(()), fail),
        (FieldKind::Secret, _) => Ok(()),
    }
}

/// Perche' `value` non e' un `{ channel, thread_id }` valido.
fn thread_ref_problem(value: &Value) -> Option<&'static str> {
    const SHAPE: &str = "deve essere { channel, thread_id }";
    let Value::Object(map) = value else { return Some(SHAPE) };
    let (Some(channel), Some(thread)) =
        (map.get("channel").and_then(Value::as_str), map.get("thread_id").and_then(Value::as_str))
    else {
        return Some(SHAPE);
    };
    if map.len() != 2 {
        Some(SHAPE)
    } else if Channel::parse(channel).is_err() {
        Some("canale non valido")
    } else if !valid_ref_id(thread) {
        Some("id del thread non valido")
    } else {
        None
    }
}
