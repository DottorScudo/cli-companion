//! Gli allegati di Telegram (puri): quali file di un messaggio in entrata si
//! offrono al runtime e come si legge la risposta di `getFile` (percorso del file
//! validato prima di finire in un URL, dimensione entro il tetto dell'HUB).

use crate::connectors::contracts::{ConnectorError, RemoteFile};
use crate::hub::attachments::ATTACHMENT_SIZE_MAX;
use serde_json::Value;

/// Lunghezza massima del `file_path` restituito da Telegram.
const FILE_PATH_MAX: usize = 256;
/// Lunghezza massima, in caratteri, del nome di un documento.
const FILE_NAME_MAX: usize = 255;

/// Cio' che serve a scaricare un file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub path: String,
    pub size: Option<u64>,
}

/// `[A-Za-z0-9._/-]{1,256}`, senza segmenti vuoti o `..` e senza `/` iniziale.
pub fn valid_file_path(path: &str) -> bool {
    let chars_ok = path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'));
    let segments_ok = path.split('/').all(|segment| !segment.is_empty() && segment != "..");
    !path.is_empty() && path.len() <= FILE_PATH_MAX && chars_ok && segments_ok
}

/// Percorso e dimensione dalla risposta di `getFile`.
pub fn parse_file_info(result: &Value) -> Result<FileInfo, ConnectorError> {
    let path = result
        .get("file_path")
        .and_then(Value::as_str)
        .filter(|path| valid_file_path(path))
        .ok_or_else(|| ConnectorError::Transient("percorso del file non valido".to_string()))?;
    let size = result.get("file_size").and_then(Value::as_u64);
    if size.is_some_and(|size| size > ATTACHMENT_SIZE_MAX) {
        return Err(ConnectorError::Transient("file troppo grande".to_string()));
    }
    Ok(FileInfo { path: path.to_string(), size })
}

fn id_of(file: &Value) -> Option<&str> {
    file.get("file_id")?.as_str().filter(|id| !id.is_empty())
}

/// La foto piu' grande fra le dimensioni offerte da Telegram (per peso, poi per area).
fn largest_photo(photo: &Value, message_id: i64) -> Option<RemoteFile> {
    let area = |size: &Value| {
        let side = |key| size.get(key).and_then(Value::as_u64).unwrap_or(0);
        side("width").saturating_mul(side("height"))
    };
    let weight = |size: &Value| size.get("file_size").and_then(Value::as_u64).unwrap_or(0);
    let best =
        photo.as_array()?.iter().filter(|size| id_of(size).is_some()).max_by_key(|size| (weight(size), area(size)))?;
    Some(RemoteFile {
        id: id_of(best)?.to_string(),
        name: format!("foto-{message_id}.jpg"),
        size: best.get("file_size").and_then(Value::as_u64),
    })
}

fn document(document: &Value, message_id: i64) -> Option<RemoteFile> {
    let name = document
        .get("file_name")
        .and_then(Value::as_str)
        .map(|name| name.chars().take(FILE_NAME_MAX).collect::<String>())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("documento-{message_id}"));
    Some(RemoteFile { id: id_of(document)?.to_string(), name, size: document.get("file_size").and_then(Value::as_u64) })
}

/// Gli allegati di un messaggio: la foto piu' grande e il documento.
pub fn message_files(message: &Value, message_id: i64) -> Vec<RemoteFile> {
    let photo = message.get("photo").and_then(|photo| largest_photo(photo, message_id));
    let doc = message.get("document").and_then(|doc| document(doc, message_id));
    photo.into_iter().chain(doc).collect()
}

#[cfg(test)]
mod tests;
