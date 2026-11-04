//! Lettura tollerante di `hub_settings.json`: un valore che questa versione non
//! conosce (scritto da una versione futura) non deve rendere illeggibile il
//! file, che altrimenti finirebbe in un backup con tutte le impostazioni.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Il valore, oppure il suo default se non si riconosce (con un avviso nel log).
pub fn or_default<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    or_else(de, T::default)
}

/// Il valore, oppure `fallback()` se non si riconosce (con un avviso nel log).
pub fn or_else<'de, D, T>(de: D, fallback: impl FnOnce() -> T) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = Value::deserialize(de)?;
    Ok(serde_json::from_value(raw).unwrap_or_else(|e| {
        log::warn!("hub_settings.json: valore non riconosciuto, uso quello di riserva: {e}");
        fallback()
    }))
}

/// Gli elementi riconosciuti di una lista; gli altri si saltano con un avviso
/// nel log. Il resto della lista e del file resta com'e'.
pub fn known_items<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = Vec::<Value>::deserialize(de)?;
    Ok(raw
        .into_iter()
        .filter_map(|item| match serde_json::from_value(item) {
            Ok(known) => Some(known),
            Err(e) => {
                log::warn!("hub_settings.json: elemento non riconosciuto ignorato: {e}");
                None
            }
        })
        .collect())
}
