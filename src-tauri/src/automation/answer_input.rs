//! Come la risposta dell'utente a una domanda del worker diventa tasti nel suo
//! terminale. Funzioni pure.
//!
//! Le due forme sono input standard di un terminale (digitare e l'incolla con
//! "bracketed paste" di xterm) e sono supportate dai coding agent TUI che l'app
//! integra: non serve un contratto. Se un agente avesse bisogno di tasti diversi,
//! si aggiunge un contratto in `agents/contracts/` invece di distinguere l'agente
//! nel core.

/// Inizio e fine dell'incolla: i ritorni a capo dentro non sono invii.
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// I tasti di una risposta, in due scritture: il testo e poi l'invio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keystrokes {
    pub text: Vec<u8>,
    pub submit: Vec<u8>,
}

/// Ripulisce il testo scritto dall'utente: a capo normalizzati a `\n`, tabulazioni
/// in spazi, via ogni altro carattere di controllo (ESC compreso: niente sequenze
/// di escape, nemmeno quella che chiude l'incolla) e spazi ai bordi. Il testo e'
/// input dell'utente scritto cosi' com'e': non si interpreta mai.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\t' => out.push(' '),
            '\n' => out.push('\n'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

/// I tasti per `clean` (gia' passato da `sanitize`): una riga si digita com'e',
/// piu' righe si incollano, cosi' i ritorni a capo non inviano a meta'.
pub fn keystrokes(clean: &str) -> Keystrokes {
    let text = if clean.contains('\n') {
        [PASTE_START, clean.as_bytes(), PASTE_END].concat()
    } else {
        clean.as_bytes().to_vec()
    };
    Keystrokes { text, submit: b"\r".to_vec() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_escape_and_c1_are_removed() {
        assert_eq!(sanitize("a\x00b\x07c\x7fd\u{85}e\x1b[31mf"), "abcde[31mf");
        assert_eq!(sanitize("\x1b]0;title\x07ok"), "]0;titleok");
    }

    #[test]
    fn line_breaks_are_normalized_and_tabs_become_spaces() {
        assert_eq!(sanitize("a\r\nb\rc\nd"), "a\nb\nc\nd");
        assert_eq!(sanitize("a\tb"), "a b");
        assert_eq!(sanitize("a\r\n\r\nb"), "a\n\nb");
    }

    #[test]
    fn unicode_is_kept_and_the_edges_are_trimmed() {
        assert_eq!(sanitize("  Sì, città \u{1F600}  "), "Sì, città \u{1F600}");
        assert_eq!(sanitize("\n\t \r\n"), "");
        assert_eq!(sanitize("\x1b\x07"), "");
    }

    #[test]
    fn a_single_line_is_typed_as_it_is() {
        let k = keystrokes("yes");
        assert_eq!(k.text, b"yes");
        assert_eq!(k.submit, b"\r");
    }

    #[test]
    fn several_lines_are_pasted_so_the_new_lines_do_not_submit() {
        let k = keystrokes("one\ntwo");
        assert_eq!(k.text, b"\x1b[200~one\ntwo\x1b[201~");
        assert_eq!(k.submit, b"\r");
    }

    #[test]
    fn an_injected_paste_end_cannot_close_the_paste_early() {
        let clean = sanitize("a\n\x1b[201~rm -rf /\n");
        let k = keystrokes(&clean);
        let text = String::from_utf8(k.text).unwrap();
        assert_eq!(text.matches('\x1b').count(), 2);
        assert!(text.ends_with("\x1b[201~"));
        assert_eq!(text.matches("\x1b[201~").count(), 1);
    }
}
