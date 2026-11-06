//! Un testo piu' lungo di un messaggio diviso in messaggi consecutivi, invece di
//! tagliarlo: ogni parte sta in `BODY_MAX` caratteri e si apre con
//! l'etichetta "(parte i/n)" su una riga sua. Il taglio cade preferibilmente
//! tra due paragrafi, poi tra due righe, poi tra due parole, mai a meta' di un
//! carattere e, se si puo', fuori da un blocco di codice Markdown; un blocco
//! tagliato si chiude alla fine della parte e si riapre nella successiva. Lo
//! spazio bianco del punto di taglio passa all'inizio della parte successiva
//! (dopo l'etichetta): togliendo le etichette e concatenando le parti si
//! ricostruisce il testo esatto, se nessun blocco di codice e' stato tagliato.
//! Oltre `MAX_PARTS` parti l'ultima finisce con un avviso che dice quanti
//! caratteri mancano e dove leggere il testo intero (`Remainder`).

use super::BODY_MAX;
use crate::validate::clip_chars;

/// Messaggi al massimo per un testo diviso.
pub const MAX_PARTS: usize = 8;
/// Caratteri dell'etichetta "(parte i/n)" con il suo a capo (12 con `MAX_PARTS` = 8).
const LABEL_RESERVE: usize = 16;
/// Lunghezza massima della sequenza di apertura di un blocco di codice (```` ``` ````)
/// che si sa chiudere e riaprire; una sequenza piu' lunga non si considera un blocco.
const FENCE_MARKER_MAX: usize = 16;
/// Caratteri tenuti dell'info (il linguaggio) di un blocco riaperto.
const FENCE_INFO_KEPT: usize = 32;
/// Caratteri della chiusura ("\n" + sequenza) e della riapertura (sequenza,
/// info, "\n") di un blocco di codice tagliato.
const FENCE_RESERVE: usize = (1 + FENCE_MARKER_MAX) + (FENCE_MARKER_MAX + FENCE_INFO_KEPT + 1);
/// Caratteri dell'avviso del testo che non ci sta (`Remainder::notice`) col suo a capo.
const NOTICE_RESERVE: usize = 200;
/// Caratteri del testo originale in una parte.
const PART_TEXT_MAX: usize = BODY_MAX - LABEL_RESERVE - FENCE_RESERVE - NOTICE_RESERVE;
/// Di quanti caratteri, al massimo, una parte si accorcia per tagliare su un
/// confine (paragrafo, riga, parola) invece che a meta' di una parola.
const BOUNDARY_WINDOW: usize = 1000;
/// Il testo piu' lungo che sta sempre in `MAX_PARTS` parti: ogni parte tranne
/// l'ultima tiene almeno `PART_TEXT_MAX - BOUNDARY_WINDOW` caratteri.
pub const LONG_BODY_MAX: usize = MAX_PARTS * (PART_TEXT_MAX - BOUNDARY_WINDOW);

/// Dove si legge il testo intero quando non sta in `MAX_PARTS` parti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remainder {
    /// Da nessuna parte: il resto non e' stato salvato (la risposta di un segretario).
    Lost,
    /// Nel report del task, nel Tasks Manager.
    InTask,
}

impl Remainder {
    fn notice(self, missing: usize) -> String {
        match self {
            Self::Lost => {
                format!("… (testo interrotto: mancano {missing} caratteri, oltre il limite di {MAX_PARTS} messaggi)")
            }
            Self::InTask => {
                format!("… (mancano {missing} caratteri: il testo intero è nel report del task, nel Tasks Manager)")
            }
        }
    }
}

/// Le parti di `text` (gia' ripulito e senza spazi ai bordi), nell'ordine:
/// una sola, uguale al testo, se sta in un messaggio; vuota per un testo vuoto.
pub fn split(text: &str, remainder: Remainder) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.chars().count() <= BODY_MAX {
        return vec![text.to_string()];
    }
    let fences = fences(text);
    let mut cuts = vec![0];
    let mut start = 0;
    while cuts.len() <= MAX_PARTS && start < text.len() {
        start = match byte_after_chars(text, start, PART_TEXT_MAX) {
            None => text.len(),
            Some(limit) => cut_point(text, start, limit, &fences),
        };
        cuts.push(start);
    }
    let missing = text[start..].chars().count();
    render(text, &cuts, &fences, missing, remainder)
}

/// Le parti, con etichetta, chiusura e riapertura dei blocchi tagliati e, se
/// il testo non e' finito tutto, l'avviso in fondo all'ultima.
fn render(text: &str, cuts: &[usize], fences: &[Fence], missing: usize, remainder: Remainder) -> Vec<String> {
    let count = cuts.len() - 1;
    cuts.windows(2)
        .enumerate()
        .map(|(i, pair)| {
            let (from, to) = (pair[0], pair[1]);
            let chunk = &text[from..to];
            let mut part = format!("(parte {}/{count})\n", i + 1);
            if let Some(fence) = fence_at(fences, from) {
                part.push_str(&fence.reopen);
                if !chunk.starts_with('\n') {
                    part.push('\n');
                }
            }
            part.push_str(chunk);
            if let Some(fence) = fence_at(fences, to).filter(|_| to < text.len()) {
                part.push('\n');
                part.push_str(&fence.close);
            }
            if i + 1 == count && missing > 0 {
                part.push_str("\n\n");
                part.push_str(&remainder.notice(missing));
            }
            part
        })
        .collect()
}

/// Dove finisce la parte che comincia a `start` e non puo' passare `limit`:
/// l'ultimo confine nella finestra prima di `limit`, nell'ordine paragrafo,
/// riga, parola, prima fuori dai blocchi di codice e poi dentro; se non c'e'
/// un confine, `limit`.
fn cut_point(text: &str, start: usize, limit: usize, fences: &[Fence]) -> usize {
    let floor = byte_after_chars(text, start, PART_TEXT_MAX - BOUNDARY_WINDOW).unwrap_or(limit);
    let window = &text[floor..limit];
    let candidates = |boundary: Boundary| {
        boundary
            .positions(window)
            .map(move |at| floor + at)
            .map(|at| text[..at].trim_end().len())
            .filter(move |&at| at >= floor)
    };
    let boundaries = [Boundary::Paragraph, Boundary::Line, Boundary::Word];
    let outside = boundaries.iter().find_map(|&b| candidates(b).find(|&at| fence_at(fences, at).is_none()));
    outside.or_else(|| boundaries.iter().find_map(|&b| candidates(b).next())).unwrap_or(limit)
}

#[derive(Debug, Clone, Copy)]
enum Boundary {
    Paragraph,
    Line,
    Word,
}

impl Boundary {
    /// Le posizioni (in byte) del confine in `window`, dall'ultima.
    fn positions(self, window: &str) -> Box<dyn Iterator<Item = usize> + '_> {
        match self {
            Self::Paragraph => Box::new(window.rmatch_indices("\n\n").map(|(at, _)| at)),
            Self::Line => Box::new(window.rmatch_indices('\n').map(|(at, _)| at)),
            Self::Word => Box::new(window.rmatch_indices([' ', '\t']).map(|(at, _)| at)),
        }
    }
}

/// Un blocco di codice delimitato (```` ``` ```` o `~~~`): il taglio in un
/// punto fra l'inizio della riga di apertura (escluso) e l'inizio della riga di
/// chiusura (o la fine del testo, se non si chiude) e' dentro il blocco.
#[derive(Debug)]
struct Fence {
    open_start: usize,
    close_start: usize,
    /// La riga di apertura da ripetere nella parte successiva (senza a capo).
    reopen: String,
    /// La sequenza che chiude il blocco.
    close: String,
}

fn fence_at(fences: &[Fence], at: usize) -> Option<&Fence> {
    fences.iter().find(|f| f.open_start < at && at < f.close_start)
}

/// I blocchi di codice di `text`, nell'ordine.
fn fences(text: &str) -> Vec<Fence> {
    let mut found = Vec::new();
    let mut open: Option<(usize, FenceLine)> = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let line_start = at;
        at += line.len();
        let Some(fence) = FenceLine::parse(line) else { continue };
        open = match open.take() {
            None => Some((line_start, fence)),
            Some((open_start, opening)) if fence.closes(&opening) => {
                found.push(opening.into_fence(open_start, line_start));
                None
            }
            still_open => still_open,
        };
    }
    if let Some((open_start, opening)) = open {
        found.push(opening.into_fence(open_start, text.len()));
    }
    found
}

/// Una riga che apre o chiude un blocco di codice: fino a tre spazi, poi
/// almeno tre ```` ` ```` o `~` uguali (al massimo `FENCE_MARKER_MAX`).
struct FenceLine {
    marker: char,
    length: usize,
    info: String,
}

impl FenceLine {
    fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\n', '\r']);
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent > 3 {
            return None;
        }
        let rest = &line[indent..];
        let marker = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        let length = rest.chars().take_while(|&c| c == marker).count();
        if !(3..=FENCE_MARKER_MAX).contains(&length) {
            return None;
        }
        let info = rest[length..].trim().to_string();
        Some(Self { marker, length, info })
    }

    /// Chiude il blocco `opening`: stessa sequenza, almeno altrettanto lunga, senza info.
    fn closes(&self, opening: &Self) -> bool {
        self.marker == opening.marker && self.length >= opening.length && self.info.is_empty()
    }

    fn into_fence(self, open_start: usize, close_start: usize) -> Fence {
        let close = self.marker.to_string().repeat(self.length);
        let reopen = format!("{close}{}", clip_chars(&self.info, FENCE_INFO_KEPT));
        Fence { open_start, close_start, reopen, close }
    }
}

/// Il byte dopo `count` caratteri da `start`; `None` se il testo finisce prima.
fn byte_after_chars(text: &str, start: usize, count: usize) -> Option<usize> {
    text[start..].char_indices().nth(count).map(|(at, _)| start + at)
}

#[cfg(test)]
mod tests;
