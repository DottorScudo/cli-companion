//! Il nome di un allegato, ripulito perche' sia un nome di file valido su
//! Windows (e quindi ovunque). Puro.

/// Lunghezza del nome di un allegato dopo la pulizia, in caratteri.
pub const NAME_MAX: usize = 120;

/// Caratteri vietati nei nomi di file di Windows: diventano `_`.
const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const REPLACEMENT: char = '_';

/// Nomi riservati da Windows (con o senza estensione, maiuscole o minuscole);
/// anche le varianti con le cifre in apice.
const RESERVED: [&str; 28] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²",
    "COM³", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

/// Nome di un allegato da quello del file scelto: solo l'ultimo componente del
/// percorso, caratteri vietati e di controllo sostituiti da `_`, niente spazi
/// iniziali ne' punti e spazi finali (Windows li toglie), un nome riservato
/// preceduto da `_`; l'estensione resta. Rifiutato se vuoto o piu' lungo di
/// `NAME_MAX` caratteri.
pub fn clean(raw: &str) -> Result<String, String> {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let replaced: String =
        base.chars().map(|c| if c.is_control() || FORBIDDEN.contains(&c) { REPLACEMENT } else { c }).collect();
    let trimmed = replaced.trim_start().trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return Err("il nome del file non è valido".to_string());
    }
    let name = if is_reserved(trimmed) { format!("{REPLACEMENT}{trimmed}") } else { trimmed.to_string() };
    if name.chars().count() > NAME_MAX {
        return Err(format!("il nome del file supera i {NAME_MAX} caratteri: rinominalo prima di allegarlo"));
    }
    Ok(name)
}

/// `CON`, `con.txt`, `Lpt1 .tar.gz`: Windows guarda la parte prima del primo punto.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_and_extensions_are_kept() {
        assert_eq!(clean("Rapporto finale.pdf").unwrap(), "Rapporto finale.pdf");
        assert_eq!(clean(".env").unwrap(), ".env");
        assert_eq!(clean("archivio.tar.gz").unwrap(), "archivio.tar.gz");
        assert_eq!(clean("città – è.png").unwrap(), "città – è.png");
    }

    #[test]
    fn only_the_last_path_component_is_kept() {
        assert_eq!(clean("C:\\Users\\me\\foto.jpg").unwrap(), "foto.jpg");
        assert_eq!(clean("../../etc/passwd").unwrap(), "passwd");
        assert_eq!(clean("a/b\\c.txt").unwrap(), "c.txt");
    }

    #[test]
    fn forbidden_and_control_characters_are_replaced() {
        assert_eq!(clean("a<b>c:d\"e|f?g*.txt").unwrap(), "a_b_c_d_e_f_g_.txt");
        assert_eq!(clean("riga\nnuova\u{0}.md").unwrap(), "riga_nuova_.md");
    }

    #[test]
    fn leading_spaces_and_trailing_dots_and_spaces_are_removed() {
        assert_eq!(clean("  nota.txt . . ").unwrap(), "nota.txt");
        for bad in ["", "   ", ".", "..", "...", " . ", "dir/", "dir\\"] {
            assert!(clean(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn reserved_names_get_a_prefix() {
        assert_eq!(clean("CON").unwrap(), "_CON");
        assert_eq!(clean("con.txt").unwrap(), "_con.txt");
        assert_eq!(clean("Lpt9 .tar.gz").unwrap(), "_Lpt9 .tar.gz");
        assert_eq!(clean("COM¹.log").unwrap(), "_COM¹.log");
        assert_eq!(clean("nul.").unwrap(), "_nul");
        assert_eq!(clean("console.txt").unwrap(), "console.txt");
        assert_eq!(clean("COM10").unwrap(), "COM10");
    }

    #[test]
    fn length_is_bounded_in_characters() {
        let max = format!("{}.txt", "é".repeat(NAME_MAX - 4));
        assert_eq!(clean(&max).unwrap(), max);
        let long = format!("{}.txt", "é".repeat(NAME_MAX - 3));
        assert!(clean(&long).unwrap_err().contains("120"));
    }

    #[test]
    fn a_clean_name_is_stable() {
        for raw in ["a<b.txt", "CON", " x. ", "C:\\y\\z.png"] {
            let once = clean(raw).unwrap();
            assert_eq!(clean(&once).unwrap(), once, "{raw:?}");
        }
    }
}
