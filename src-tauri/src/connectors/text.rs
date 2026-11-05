//! Pulizia del testo che arriva da un servizio esterno (pura): prima di finire in un
//! DTO, in un titolo o in un messaggio i testi perdono caratteri di controllo e
//! invisibili e vengono tagliati. Condivisa fra i connettori.

/// Caratteri invisibili che riordinano o nascondono il testo (direzione, larghezza zero).
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// `raw` senza caratteri di controllo ne' invisibili, senza spazi ai bordi, al piu' `max` caratteri.
pub fn clean_display(raw: &str, max: usize) -> String {
    let kept: String =
        raw.chars().filter(|c| !c.is_control() && !is_invisible(*c)).take(max.saturating_mul(2)).collect();
    kept.trim().chars().take(max).collect()
}

/// `raw` su una sola riga: ogni sequenza di spazi, a capo e caratteri di controllo
/// diventa un solo spazio, gli invisibili spariscono, niente spazi ai bordi, al piu'
/// `max` caratteri (il taglio cade sempre su un confine di carattere).
pub fn single_line(raw: &str, max: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    let mut pending_space = false;
    for c in raw.chars() {
        if is_invisible(c) {
            continue;
        }
        if c.is_whitespace() || c.is_control() {
            pending_space = count > 0;
            continue;
        }
        let needed = if pending_space { 2 } else { 1 };
        if count.saturating_add(needed) > max {
            break;
        }
        if pending_space {
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        out.push(c);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_display_trims_and_caps() {
        assert_eq!(clean_display("  ciao  ", 10), "ciao");
        assert_eq!(clean_display("abcdef", 3), "abc");
        assert_eq!(clean_display("\u{0}\u{1}", 3), "");
    }

    #[test]
    fn single_line_collapses_every_kind_of_whitespace() {
        assert_eq!(single_line("  a \n\t b\r\nc  ", 50), "a b c");
        assert_eq!(single_line("a\u{0}b", 50), "a b");
        assert_eq!(single_line("\u{202e}Ro\u{200b}ssi", 50), "Rossi");
    }

    #[test]
    fn single_line_handles_empty_and_blank_input() {
        assert_eq!(single_line("", 10), "");
        assert_eq!(single_line(" \n\t ", 10), "");
        assert_eq!(single_line("abc", 0), "");
    }

    #[test]
    fn single_line_cuts_on_character_boundaries_without_trailing_space() {
        assert_eq!(single_line(&"😀".repeat(100), 5).chars().count(), 5);
        assert_eq!(single_line("ab cd", 3), "ab");
        assert_eq!(single_line("ab cd", 4), "ab c");
        assert_eq!(single_line("ab cd", 5), "ab cd");
        assert_eq!(single_line(&"a ".repeat(10_000), 7), "a a a a");
    }
}
