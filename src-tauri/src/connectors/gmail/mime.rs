//! L'albero delle parti MIME di un messaggio come lo restituisce `messages.get`
//! (`format=full`, puro): il testo (`text/plain` o `text/html`) e gli allegati.
//! La visita ha tetti di profondita' e di parti: un messaggio ostile non la dilata.

use super::charset::{decode_base64url, decode_or_utf8};
use super::words::decode_words;
use crate::connectors::text::single_line;
use serde_json::Value;

/// Profondita' massima dell'albero delle parti.
pub const DEPTH_MAX: usize = 20;
/// Parti visitate al massimo.
pub const PARTS_MAX: usize = 500;
/// Allegati raccolti al massimo.
pub const ATTACHMENTS_MAX: usize = 50;
/// Lunghezza massima, in caratteri, del nome di un allegato.
const FILENAME_MAX: usize = 255;
/// Caratteri base64 letti da una parte di testo: ne basta una parte per un testo da 32.000 caratteri.
/// Multiplo di 4, cosi' il taglio cade fra gruppi base64.
const TEXT_B64_MAX: usize = 512 * 1024;

/// Dove si prende il contenuto di un allegato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentSource {
    /// L'`attachmentId` da chiedere a `messages.attachments.get`.
    Remote(String),
    /// I byte, gia' nel messaggio.
    Inline(Vec<u8>),
}

/// Un allegato di un messaggio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    pub filename: String,
    pub size: u64,
    pub source: AttachmentSource,
}

/// Cio' che serve di un albero di parti.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Collected {
    /// La prima parte `text/plain` che non e' un allegato.
    pub plain: Option<String>,
    /// La prima parte `text/html` che non e' un allegato.
    pub html: Option<String>,
    pub attachments: Vec<AttachmentRef>,
}

/// Il valore di un'intestazione (nome senza distinzione di maiuscole) di un nodo con `headers`.
pub fn header_value<'a>(node: &'a Value, name: &str) -> Option<&'a str> {
    node.get("headers")?
        .as_array()?
        .iter()
        .find(|h| h.get("name").and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(name)))?
        .get("value")?
        .as_str()
}

/// Il nome di un allegato: parole codificate decodificate, su una riga, tagliato.
fn clean_name(raw: &str) -> String {
    single_line(&decode_words(raw), FILENAME_MAX)
}

/// Il parametro `charset` di un `Content-Type` (vuoto se manca).
fn charset_of(content_type: &str) -> &str {
    let lower = content_type.to_ascii_lowercase();
    let Some(at) = lower.find("charset=") else {
        return "";
    };
    let value = content_type.get(at + "charset=".len()..).unwrap_or("");
    value.split(';').next().unwrap_or("").trim().trim_matches('"')
}

fn is_attachment_disposition(part: &Value) -> bool {
    header_value(part, "Content-Disposition")
        .is_some_and(|d| d.trim_start().to_ascii_lowercase().starts_with("attachment"))
}

/// Il testo di una parte, decodificato col suo set di caratteri.
fn text_of(part: &Value) -> Option<String> {
    let data = part.get("body")?.get("data")?.as_str()?;
    let data = if data.len() > TEXT_B64_MAX { data.get(..TEXT_B64_MAX)? } else { data };
    let bytes = decode_base64url(data)?;
    let charset = header_value(part, "Content-Type").map(charset_of).unwrap_or("");
    Some(decode_or_utf8(&bytes, charset))
}

/// L'allegato di una parte con nome: da scaricare (`attachmentId`) o gia' nel messaggio.
fn attachment_of(part: &Value, filename: String) -> Option<AttachmentRef> {
    let body = part.get("body")?;
    let declared = body.get("size").and_then(Value::as_u64);
    if let Some(id) = body.get("attachmentId").and_then(Value::as_str).filter(|id| !id.is_empty()) {
        return Some(AttachmentRef {
            filename,
            size: declared.unwrap_or(0),
            source: AttachmentSource::Remote(id.to_string()),
        });
    }
    let bytes = decode_base64url(body.get("data")?.as_str()?)?;
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    Some(AttachmentRef { filename, size, source: AttachmentSource::Inline(bytes) })
}

struct Walker {
    visited: usize,
    out: Collected,
}

impl Walker {
    fn visit(&mut self, part: &Value, depth: usize) {
        if depth > DEPTH_MAX || self.visited >= PARTS_MAX {
            return;
        }
        self.visited += 1;
        self.take(part);
        if let Some(children) = part.get("parts").and_then(Value::as_array) {
            for child in children {
                self.visit(child, depth + 1);
            }
        }
    }

    /// Smista il contenuto di una parte: allegato, testo semplice o HTML.
    fn take(&mut self, part: &Value) {
        let filename = part.get("filename").and_then(Value::as_str).map(clean_name).unwrap_or_default();
        if !filename.is_empty() {
            if self.out.attachments.len() < ATTACHMENTS_MAX {
                self.out.attachments.extend(attachment_of(part, filename));
            }
            return;
        }
        if is_attachment_disposition(part) {
            return;
        }
        let mime = part.get("mimeType").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
        let slot = match mime.as_str() {
            "text/plain" => &mut self.out.plain,
            "text/html" => &mut self.out.html,
            _ => return,
        };
        if slot.is_none() {
            *slot = text_of(part);
        }
    }
}

/// Visita `payload` (la radice conta come una parte).
pub fn collect(payload: &Value) -> Collected {
    let mut walker = Walker { visited: 0, out: Collected::default() };
    walker.visit(payload, 0);
    walker.out
}

#[cfg(test)]
mod tests;
