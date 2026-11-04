//! Validazione dei testi e degli id che entrano nell'HUB. Funzioni pure con
//! messaggi in italiano: li legge l'utente nell'interfaccia.

/// Lunghezza del titolo di un thread, in caratteri.
pub const TITLE_MAX: usize = 120;
/// Lunghezza del testo di un messaggio (e del riassunto di un evento), in caratteri.
pub const BODY_MAX: usize = 8000;
/// Lunghezza del nome di un bot, in caratteri (come i nomi della pool di `bots.rs`).
pub const BOT_NAME_MAX: usize = 24;
/// Lunghezza di un id di riferimento (proposta, stato), in byte.
const REF_ID_MAX: usize = 64;

/// Titolo di un thread: trim, 1..=`TITLE_MAX` caratteri, una riga sola.
pub fn title(raw: &str) -> Result<String, String> {
    single_line("titolo", raw, TITLE_MAX)
}

/// Nome di un bot autore: trim, 1..=`BOT_NAME_MAX` caratteri, una riga sola.
pub fn bot_name(raw: &str) -> Result<String, String> {
    single_line("nome del bot", raw, BOT_NAME_MAX)
}

/// Testo di un messaggio: trim, non vuoto, al massimo `BODY_MAX` caratteri.
/// Sono ammessi a capo e tabulazioni, non gli altri caratteri di controllo.
pub fn body(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("il messaggio è vuoto".to_string());
    }
    if text.chars().count() > BODY_MAX {
        return Err(format!("il messaggio supera i {BODY_MAX} caratteri"));
    }
    if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("il messaggio contiene caratteri non validi".to_string());
    }
    Ok(text.to_string())
}

/// Id di un riferimento generato dall'app (proposta) o di un valore breve
/// (stato di un task): 1..=64 caratteri ASCII tra lettere, cifre, `-` e `_`.
pub fn valid_ref_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= REF_ID_MAX && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn single_line(label: &str, raw: &str, max: usize) -> Result<String, String> {
    let text = raw.trim();
    let chars = text.chars().count();
    if chars == 0 || chars > max {
        return Err(format!("il {label} deve avere da 1 a {max} caratteri"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!("il {label} contiene caratteri non validi"));
    }
    Ok(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_and_bounded() {
        assert_eq!(title("  Rilascio 1.0 ").unwrap(), "Rilascio 1.0");
        assert_eq!(title(&"é".repeat(TITLE_MAX)).unwrap().chars().count(), TITLE_MAX);
        for bad in ["", "   ", "a\nb", &"x".repeat(TITLE_MAX + 1)] {
            assert!(title(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn bodies_keep_newlines_and_reject_control_characters() {
        assert_eq!(body("  riga 1\nriga 2\t \n").unwrap(), "riga 1\nriga 2");
        assert_eq!(body(&"€".repeat(BODY_MAX)).unwrap().chars().count(), BODY_MAX);
        assert_eq!(body(" \n ").unwrap_err(), "il messaggio è vuoto");
        assert!(body(&"x".repeat(BODY_MAX + 1)).unwrap_err().contains("8000"));
        assert!(body("a\u{0}b").is_err());
        assert!(body("a\u{1b}[31mb").is_err());
    }

    #[test]
    fn bot_names_are_bounded() {
        assert_eq!(bot_name(" mario ").unwrap(), "mario");
        assert!(bot_name("").is_err() && bot_name(&"m".repeat(BOT_NAME_MAX + 1)).is_err());
    }

    #[test]
    fn ref_ids() {
        assert!(valid_ref_id("p-1700000000000-3") && valid_ref_id(&"a".repeat(64)));
        for bad in ["", "a b", "../x", "è", &"a".repeat(65)] {
            assert!(!valid_ref_id(bad), "{bad:?}");
        }
    }
}
