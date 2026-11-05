//! Titolo, mittente e corpo del thread di un'email (puri). Il corpo e' pronto per l'HUB:
//! la riga degli allegati non importati sta nell'intestazione, cosi' il taglio del testo
//! lungo non la porta via.

use crate::connectors::text::single_line;
use chrono::DateTime;

/// Lunghezza massima, in caratteri, del titolo di un thread e del nome di un mittente (limite dell'HUB).
pub const TITLE_MAX: usize = 120;
/// Lunghezza massima dell'oggetto mostrato nel corpo (intero, a differenza del titolo).
const SUBJECT_BODY_MAX: usize = 1000;
/// Lunghezza massima dell'intestazione `Date` mostrata.
pub const DATE_MAX: usize = 100;
/// Allegati non importati elencati per nome; gli altri sono «e altri N».
const NOT_IMPORTED_NAMES_MAX: usize = 10;
/// Lunghezza massima del nome di un allegato non importato nell'elenco.
const NOT_IMPORTED_NAME_MAX: usize = 80;
/// Lunghezza massima di un indirizzo email.
const ADDRESS_MAX: usize = 254;
const UNKNOWN_SENDER: &str = "Mittente sconosciuto";
const NO_SUBJECT: &str = "(senza oggetto)";
const UNKNOWN_DATE: &str = "data sconosciuta";
const KIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;
/// Decimi in un'unita'.
const TENTHS: u64 = 10;

/// Nome e indirizzo di `Nome <indirizzo>`; senza parentesi angolari e' tutto indirizzo.
fn split_from(from: &str) -> (String, String) {
    let trimmed = from.trim();
    match (trimmed.rfind('<'), trimmed.ends_with('>')) {
        (Some(open), true) => {
            let name = trimmed.get(..open).unwrap_or("").trim().trim_matches('"');
            let address = trimmed.get(open + 1..trimmed.len() - 1).unwrap_or("");
            (single_line(name, TITLE_MAX), single_line(address, ADDRESS_MAX))
        }
        _ => (String::new(), single_line(trimmed, ADDRESS_MAX)),
    }
}

/// Il mittente come si presenta: `Mario Rossi <mario@gmail.com>`, o solo l'indirizzo.
pub fn sender_name(from_header: &str) -> String {
    let (name, address) = split_from(from_header);
    let full = match (name.is_empty(), address.is_empty()) {
        (true, true) => return UNKNOWN_SENDER.to_string(),
        (true, false) => address,
        (false, true) => name,
        (false, false) => format!("{name} <{address}>"),
    };
    single_line(&full, TITLE_MAX)
}

/// Il titolo del thread: l'oggetto su una riga, al piu' 120 caratteri.
pub fn thread_title(subject: &str) -> String {
    let title = single_line(subject, TITLE_MAX);
    if title.is_empty() {
        NO_SUBJECT.to_string()
    } else {
        title
    }
}

/// Un numero con un decimale solo se serve, virgola italiana: `12,3`, `40`.
fn tenths_text(tenths: u64) -> String {
    match (tenths / TENTHS, tenths % TENTHS) {
        (whole, 0) => whole.to_string(),
        (whole, decimal) => format!("{whole},{decimal}"),
    }
}

/// La dimensione leggibile: `512 B`, `3,5 KB`, `12,3 MB`.
pub fn size_text(bytes: u64) -> String {
    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{} KB", tenths_text(bytes.saturating_mul(TENTHS) / KIB))
    } else {
        format!("{} MB", tenths_text(bytes.saturating_mul(TENTHS) / MIB))
    }
}

/// `Allegati non importati: a.pdf (12,3 MB), b.zip (40 MB)`.
fn not_imported_line(not_imported: &[(String, u64)]) -> Option<String> {
    if not_imported.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = not_imported
        .iter()
        .take(NOT_IMPORTED_NAMES_MAX)
        .map(|(name, size)| format!("{} ({})", single_line(name, NOT_IMPORTED_NAME_MAX), size_text(*size)))
        .collect();
    if not_imported.len() > NOT_IMPORTED_NAMES_MAX {
        parts.push(format!("e altri {}", not_imported.len() - NOT_IMPORTED_NAMES_MAX));
    }
    Some(format!("Allegati non importati: {}", parts.join(", ")))
}

/// Il corpo del thread: intestazione (oggetto, mittente, data, allegati non importati), riga vuota, testo.
pub fn email_body(subject: &str, sender: &str, date: &str, not_imported: &[(String, u64)], text: &str) -> String {
    let mut lines = vec![
        format!("Oggetto: {}", thread_title_full(subject)),
        format!("Da: {}", single_line(sender, TITLE_MAX)),
        format!("Data: {}", single_line(date, DATE_MAX)),
    ];
    lines.extend(not_imported_line(not_imported));
    format!("{}\n\n{text}", lines.join("\n"))
}

/// L'oggetto per il corpo: su una riga, ma senza il taglio del titolo.
fn thread_title_full(subject: &str) -> String {
    let clean = single_line(subject, SUBJECT_BODY_MAX);
    if clean.is_empty() {
        NO_SUBJECT.to_string()
    } else {
        clean
    }
}

/// La data da mostrare: l'intestazione `Date` ripulita o, se manca, `internal_ms` in UTC.
pub fn date_label(date_header: &str, internal_ms: i64) -> String {
    let cleaned = single_line(date_header, DATE_MAX);
    if !cleaned.is_empty() {
        return cleaned;
    }
    DateTime::from_timestamp_millis(internal_ms)
        .map_or_else(|| UNKNOWN_DATE.to_string(), |d| d.format("%d/%m/%Y %H:%M UTC").to_string())
}

#[cfg(test)]
mod tests;
