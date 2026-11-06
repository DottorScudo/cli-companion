//! Il testo in uscita (markdown dei modelli o testo semplice dell'app) come messaggi
//! HTML di Telegram (puro). `render` legge il markdown in tratti di testo visibile con
//! i loro tag, `text` sceglie dove tagliare il testo visibile (al piu' `PARTS_MAX`
//! parti di 4096 unita' UTF-16, misurate senza tag ne' entita'), `markup` scrive l'HTML
//! di ogni parte: ben formato, con i tag aperti al taglio chiusi in fondo e riaperti
//! (con gli stessi attributi) nella parte dopo. Il rimando al testo completo nell'HUB
//! chiude l'ultima parte fuori da ogni tag. Ogni parte porta anche il suo testo
//! visibile, per rimandarla senza formattazione se Telegram rifiuta l'HTML.

mod attributes;
mod markup;
mod render;

use super::text::{layout, TRUNCATED_MARKER};
use markup::{escape_text, html, Run, Tag};
use std::ops::Range;

/// Un messaggio pronto: l'HTML (`parse_mode: "HTML"`) e lo stesso testo senza tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub html: String,
    pub plain: String,
}

/// I messaggi di `text`, in ordine. Vuoto (o solo spazi): nessun messaggio.
pub fn parts(text: &str) -> Vec<Part> {
    let runs = render::runs(text);
    let visible: String = runs.iter().map(|run| run.text.as_str()).collect();
    let layout = layout(&visible);
    let last = layout.parts.len().saturating_sub(1);
    layout
        .parts
        .iter()
        .enumerate()
        .map(|(index, range)| {
            let marker = if layout.truncated && index == last { TRUNCATED_MARKER } else { "" };
            part(&runs, &visible, range.clone(), marker)
        })
        .collect()
}

/// La parte `range` del testo visibile, chiusa da `marker`.
fn part(runs: &[Run], visible: &str, range: Range<usize>, marker: &str) -> Part {
    let mut html = html(pieces(runs, range.clone()));
    escape_text(marker, &mut html);
    Part { html, plain: format!("{}{marker}", &visible[range]) }
}

/// I tratti (o i loro pezzi) che cadono in `range` del testo visibile.
fn pieces(runs: &[Run], range: Range<usize>) -> impl Iterator<Item = (&[Tag], &str)> {
    let mut start = 0;
    runs.iter().filter_map(move |run| {
        let (run_start, run_end) = (start, start + run.text.len());
        start = run_end;
        let from = range.start.max(run_start);
        let to = range.end.min(run_end);
        (from < to).then(|| (run.tags.as_slice(), &run.text[from - run_start..to - run_start]))
    })
}

#[cfg(test)]
mod tests;
