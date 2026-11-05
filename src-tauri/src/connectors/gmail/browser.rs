//! L'unico punto che apre il browser di sistema. L'URL contiene il client id e lo `state`:
//! non si riporta mai in un errore.

use crate::connectors::contracts::ConnectorError;

/// Apre `url` nel browser predefinito.
pub fn open(url: &str) -> Result<(), ConnectorError> {
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|_| ConnectorError::Fatal("impossibile aprire il browser".to_string()))
}
