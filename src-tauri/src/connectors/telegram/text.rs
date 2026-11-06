//! Dove spezzare un testo nei messaggi di Telegram (puro). Il limite e' di 4096 unita'
//! UTF-16 di testo visibile per messaggio (i tag HTML non contano); si taglia preferendo
//! l'ultimo a capo, poi l'ultimo spazio, mai dentro un carattere. Al piu' `PARTS_MAX`
//! parti: se il testo e' piu' lungo, l'ultima lascia il posto al rimando al testo
//! completo nell'HUB (`TRUNCATED_MARKER`). Le parti sono intervalli del testo visibile:
//! `format` le riveste con i tag che vi cadono dentro.

use std::ops::Range;

/// Il limite di Telegram per un messaggio, in unita' UTF-16.
pub const PART_UNITS_MAX: usize = 4096;
/// Messaggi al massimo per una risposta.
pub const PARTS_MAX: usize = 3;
/// Chiude l'ultima parte di un testo troncato.
pub const TRUNCATED_MARKER: &str = "\n… (testo completo nell'HUB)";

/// Le parti di un testo: intervalli di byte, in ordine, senza spazi ai bordi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub parts: Vec<Range<usize>>,
    /// Il testo non ci sta tutto: l'ultima parte va chiusa con `TRUNCATED_MARKER`.
    pub truncated: bool,
}

pub fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// Dove tagliare (indice di byte) perche' la testa stia in `limit` unita': l'ultimo a
/// capo, altrimenti l'ultimo spazio, altrimenti subito prima del carattere che sfora.
/// Se il testo sta tutto nel limite, la sua lunghezza.
fn cut_point(text: &str, limit: usize) -> usize {
    let (mut used, mut newline, mut space) = (0usize, None, None);
    for (index, c) in text.char_indices() {
        if used + c.len_utf16() > limit {
            let hard = if index == 0 { c.len_utf8() } else { index };
            return newline.or(space).unwrap_or(hard);
        }
        used += c.len_utf16();
        if index > 0 {
            match c {
                '\n' => newline = Some(index),
                ' ' => space = Some(index),
                _ => {}
            }
        }
    }
    text.len()
}

/// Gli spazi iniziali di `text`, in byte.
fn leading_space(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// La testa di `text[start..end]` che sta in `limit` unita', senza spazi finali.
fn head(text: &str, start: usize, end: usize, limit: usize) -> Range<usize> {
    let rest = &text[start..end];
    let cut = cut_point(rest, limit);
    start..start + rest[..cut].trim_end().len()
}

/// Le parti di `text`, al piu' `PARTS_MAX`. Vuoto (o solo spazi): nessuna parte.
pub fn layout(text: &str) -> Layout {
    let mut parts = Vec::new();
    let mut truncated = false;
    let end = text.trim_end().len();
    let mut start = leading_space(text);
    while start < end {
        let part = if parts.len() + 1 == PARTS_MAX {
            truncated = utf16_len(&text[start..end]) > PART_UNITS_MAX;
            if truncated {
                head(text, start, end, PART_UNITS_MAX.saturating_sub(utf16_len(TRUNCATED_MARKER)))
            } else {
                start..end
            }
        } else {
            head(text, start, end, PART_UNITS_MAX)
        };
        let next = part.end;
        parts.push(part);
        if truncated {
            break;
        }
        start = next + leading_space(&text[next..end]);
    }
    Layout { parts, truncated }
}

#[cfg(test)]
mod tests;
