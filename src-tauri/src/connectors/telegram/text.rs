//! Il testo spezzato nei messaggi di Telegram (puro). Il limite e' di 4096 unita'
//! UTF-16 per messaggio; si taglia preferendo l'ultimo a capo, poi l'ultimo spazio,
//! mai dentro un carattere. Al piu' `PARTS_MAX` parti: se il testo e' piu' lungo,
//! l'ultima finisce con un rimando al testo completo nell'HUB.

/// Il limite di Telegram per un messaggio, in unita' UTF-16.
pub const PART_UNITS_MAX: usize = 4096;
/// Messaggi al massimo per una risposta.
pub const PARTS_MAX: usize = 3;
/// Chiude l'ultima parte di un testo troncato.
const TRUNCATED_MARKER: &str = "\n… (testo completo nell'HUB)";

fn utf16_len(text: &str) -> usize {
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

/// `(testa, coda)` al punto di taglio; la coda perde gli spazi e gli a capo iniziali.
fn split_at_cut(text: &str, cut: usize) -> (&str, &str) {
    let (head, tail) = text.split_at_checked(cut).unwrap_or((text, ""));
    (head, tail.trim_start())
}

/// L'ultima parte di un testo troppo lungo: il suo inizio piu' il rimando.
fn truncated_last(rest: &str) -> String {
    let room = PART_UNITS_MAX.saturating_sub(utf16_len(TRUNCATED_MARKER));
    let (head, _) = split_at_cut(rest, cut_point(rest, room));
    format!("{}{TRUNCATED_MARKER}", head.trim_end())
}

/// Il testo in al piu' `PARTS_MAX` messaggi. Vuoto (o solo spazi): nessuna parte.
pub fn split(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if parts.len() + 1 == PARTS_MAX {
            parts.push(if utf16_len(rest) <= PART_UNITS_MAX { rest.to_string() } else { truncated_last(rest) });
            break;
        }
        let (head, tail) = split_at_cut(rest, cut_point(rest, PART_UNITS_MAX));
        if !head.trim().is_empty() {
            parts.push(head.trim_end().to_string());
        }
        rest = tail;
    }
    parts
}

#[cfg(test)]
mod tests;
