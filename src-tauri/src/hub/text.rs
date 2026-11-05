//! Validazione dei testi e degli id che entrano nell'HUB. Funzioni pure con
//! messaggi in italiano: li legge l'utente nell'interfaccia.

use crate::validate::clip_with_ellipsis;

/// Lunghezza del titolo di un thread, in caratteri.
pub const TITLE_MAX: usize = 120;
/// Lunghezza del testo di un messaggio (e del riassunto di un evento), in caratteri.
pub const BODY_MAX: usize = 8000;
/// Lunghezza del nome di un bot, in caratteri (come i nomi della pool di `bots.rs`).
pub const BOT_NAME_MAX: usize = 24;
/// Lunghezza del nome di un mittente esterno (autore `external`), in caratteri.
pub const EXTERNAL_NAME_MAX: usize = 120;
/// Lunghezza dell'id di un elemento nel servizio esterno (`MessageOrigin::external_id`), in caratteri.
pub const EXTERNAL_ID_MAX: usize = 256;
/// Il segno che il testo di un messaggio e' stato tagliato per stare nel limite.
pub const TRUNCATED_SUFFIX: &str = "… (troncato)";
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

/// Nome di un mittente esterno: i caratteri di controllo diventano spazi, poi
/// trim e 1..=`EXTERNAL_NAME_MAX` caratteri (tagliato con "…" se piu' lungo).
pub fn external_name(raw: &str) -> Result<String, String> {
    let name = clean_line(raw, EXTERNAL_NAME_MAX - 1);
    if name.is_empty() {
        return Err("il nome del mittente esterno è vuoto".to_string());
    }
    Ok(name)
}

/// Id di un elemento nel servizio esterno: 1..=`EXTERNAL_ID_MAX` caratteri, senza
/// caratteri di controllo. Non si ripulisce: l'id e' la chiave del dedupe.
pub fn external_id(raw: &str) -> Result<String, String> {
    let chars = raw.chars().count();
    if chars == 0 || chars > EXTERNAL_ID_MAX {
        return Err(format!("l'id esterno deve avere da 1 a {EXTERNAL_ID_MAX} caratteri"));
    }
    if raw.chars().any(char::is_control) {
        return Err("l'id esterno contiene caratteri non validi".to_string());
    }
    Ok(raw.to_string())
}

/// Testo di un messaggio: trim, non vuoto, al massimo `BODY_MAX` caratteri.
/// Sono ammessi a capo e tabulazioni, non gli altri caratteri di controllo.
pub fn body(raw: &str) -> Result<String, String> {
    let text = optional_body(raw)?;
    if text.is_empty() {
        return Err("il messaggio è vuoto".to_string());
    }
    Ok(text)
}

/// Come `body`, ma il testo puo' essere vuoto (un messaggio con allegati).
pub fn optional_body(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.chars().count() > BODY_MAX {
        return Err(format!("il messaggio supera i {BODY_MAX} caratteri"));
    }
    if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("il messaggio contiene caratteri non validi".to_string());
    }
    Ok(text.to_string())
}

/// Il testo di un messaggio che arriva da fuori (non fidato) e la sua eventuale
/// riga di nota, pronti per `body`: i caratteri di controllo (a capo e
/// tabulazioni esclusi) diventano spazi e il testo si taglia, con
/// `TRUNCATED_SUFFIX`, perche' con la nota (su una riga a parte, in fondo) stia
/// in `BODY_MAX` caratteri. Puo' tornare vuoto.
pub fn body_with_note(raw: &str, note: Option<&str>) -> String {
    let note_len = note.map_or(0, |n| n.chars().count() + 1);
    let text = clean_text(raw, usize::MAX);
    let budget = BODY_MAX.saturating_sub(note_len);
    let text = if text.chars().count() > budget {
        let keep = budget.saturating_sub(TRUNCATED_SUFFIX.chars().count());
        format!("{}{TRUNCATED_SUFFIX}", text.chars().take(keep).collect::<String>())
    } else {
        text
    };
    match (text.is_empty(), note) {
        (_, None) => text,
        (true, Some(note)) => note.to_string(),
        (false, Some(note)) => format!("{text}\n{note}"),
    }
}

/// Id di un riferimento generato dall'app (proposta) o di un valore breve
/// (stato di un task): 1..=64 caratteri ASCII tra lettere, cifre, `-` e `_`.
pub fn valid_ref_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= REF_ID_MAX && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Testo su una riga scritto da altri (un titolo della board) e riportato in un
/// messaggio: i caratteri di controllo diventano spazi, poi trim e al massimo
/// `max` caratteri (con "…" se tagliato). Puo' tornare vuoto.
pub fn clean_line(raw: &str, max: usize) -> String {
    clean(raw, max, |c| c.is_control())
}

/// Come `clean_line`, ma tiene a capo e tabulazioni (un riassunto, una nota).
pub fn clean_text(raw: &str, max: usize) -> String {
    clean(raw, max, |c| c.is_control() && !matches!(c, '\n' | '\t'))
}

fn clean(raw: &str, max: usize, blank: impl Fn(char) -> bool) -> String {
    let replaced: String = raw.chars().map(|c| if blank(c) { ' ' } else { c }).collect();
    clip_with_ellipsis(replaced.trim(), max)
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
    fn optional_bodies_may_be_empty_but_keep_the_other_rules() {
        assert_eq!(optional_body(" \n ").unwrap(), "");
        assert_eq!(optional_body(" ciao ").unwrap(), "ciao");
        assert!(optional_body(&"x".repeat(BODY_MAX + 1)).is_err());
        assert!(optional_body("a\u{0}b").is_err());
    }

    #[test]
    fn bot_names_are_bounded() {
        assert_eq!(bot_name(" mario ").unwrap(), "mario");
        assert!(bot_name("").is_err() && bot_name(&"m".repeat(BOT_NAME_MAX + 1)).is_err());
    }

    #[test]
    fn board_texts_are_cleaned_and_clipped() {
        assert_eq!(clean_line(" a\nb\u{1b}c ", 10), "a b c");
        assert_eq!(clean_text(" riga 1\n\triga 2\u{0} ", 20), "riga 1\n\triga 2");
        assert_eq!(clean_line(&"é".repeat(5), 3), "ééé…");
        assert_eq!(clean_text("\u{7} ", 5), "");
    }

    #[test]
    fn external_bodies_are_cleaned_and_cut_to_leave_room_for_the_note() {
        assert_eq!(body_with_note(" ciao\u{0}\nmondo ", None), "ciao \nmondo");
        assert_eq!(body_with_note("ciao", Some("nota")), "ciao\nnota");
        assert_eq!(body_with_note("  ", Some("nota")), "nota");
        assert_eq!(body_with_note("", None), "");
        let exact = "x".repeat(BODY_MAX);
        assert_eq!(body_with_note(&exact, None), exact);
        let cut = body_with_note(&format!("{exact}y"), None);
        assert_eq!(cut.chars().count(), BODY_MAX);
        assert!(cut.ends_with(TRUNCATED_SUFFIX));
        let with_note = body_with_note(&"é".repeat(BODY_MAX), Some("Allegati non importati: a.pdf (troppo grande)"));
        assert_eq!(with_note.chars().count(), BODY_MAX);
        assert!(with_note.contains(&format!("{TRUNCATED_SUFFIX}\nAllegati")));
        assert!(optional_body(&with_note).is_ok());
    }

    #[test]
    fn external_names_are_cleaned_and_must_not_be_empty() {
        assert_eq!(external_name("  Mario\nRossi\u{1b} ").unwrap(), "Mario Rossi");
        assert_eq!(external_name(&"é".repeat(EXTERNAL_NAME_MAX + 5)).unwrap().chars().count(), EXTERNAL_NAME_MAX);
        assert!(external_name(" \n\u{7} ").is_err() && external_name("").is_err());
    }

    #[test]
    fn external_ids_are_bounded_and_have_no_control_characters() {
        assert_eq!(external_id("<a@b.c>").unwrap(), "<a@b.c>");
        assert!(external_id(&"x".repeat(EXTERNAL_ID_MAX)).is_ok());
        for bad in ["", "a\nb", "a\u{0}", &"x".repeat(EXTERNAL_ID_MAX + 1)] {
            assert!(external_id(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ref_ids() {
        assert!(valid_ref_id("p-1700000000000-3") && valid_ref_id(&"a".repeat(64)));
        for bad in ["", "a b", "../x", "è", &"a".repeat(65)] {
            assert!(!valid_ref_id(bad), "{bad:?}");
        }
    }
}
