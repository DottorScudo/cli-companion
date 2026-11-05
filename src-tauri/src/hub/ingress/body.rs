//! Il testo di una consegna esterna, puro: ripulito, tagliato per fare posto alla
//! nota sugli allegati non importati e mai vuoto senza allegati.

use crate::hub::text::{body_with_note, optional_body};

const EMPTY_MESSAGE: &str = "il messaggio è vuoto";

/// Il testo del messaggio: `raw` ripulito e tagliato (`text::body_with_note`),
/// con `note` su una riga a parte in fondo. `attached`: il messaggio ha almeno
/// un allegato, quindi il testo puo' essere vuoto. Il risultato passa sempre
/// `text::optional_body`.
pub fn compose(raw: &str, note: Option<&str>, attached: bool) -> Result<String, String> {
    let text = body_with_note(raw, note);
    if text.is_empty() && !attached {
        return Err(EMPTY_MESSAGE.to_string());
    }
    optional_body(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::text::{BODY_MAX, TRUNCATED_SUFFIX};

    #[test]
    fn a_normal_body_is_cleaned() {
        assert_eq!(compose("  ciao\u{0}\nmondo ", None, false).unwrap(), "ciao \nmondo");
    }

    #[test]
    fn a_long_body_is_cut_with_the_suffix_and_stays_within_the_limit() {
        let body = compose(&"x".repeat(BODY_MAX + 500), None, false).unwrap();
        assert_eq!(body.chars().count(), BODY_MAX);
        assert!(body.ends_with(TRUNCATED_SUFFIX));
    }

    #[test]
    fn the_note_goes_last_on_its_own_line_and_the_text_makes_room() {
        let note = "Allegati non importati: a.pdf (troppo grande)";
        let body = compose(&"x".repeat(BODY_MAX), Some(note), false).unwrap();
        assert_eq!(body.chars().count(), BODY_MAX);
        assert!(body.ends_with(&format!("{TRUNCATED_SUFFIX}\n{note}")));
        assert_eq!(compose("ciao", Some(note), false).unwrap(), format!("ciao\n{note}"));
    }

    #[test]
    fn an_empty_body_needs_attachments_or_a_note() {
        assert_eq!(compose(" \n\u{7}", None, false).unwrap_err(), EMPTY_MESSAGE);
        assert_eq!(compose("", None, true).unwrap(), "");
        assert_eq!(compose("", Some("nota"), false).unwrap(), "nota");
    }
}
