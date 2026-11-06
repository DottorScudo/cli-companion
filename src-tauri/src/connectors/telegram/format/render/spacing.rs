//! Lo spazio fra due blocchi come nel testo d'origine (puro): una riga vuota se c'era,
//! altrimenti un solo a capo. Si guardano le posizioni degli eventi di `pulldown_cmark`.

use std::ops::Range;

/// Dove finisce il contenuto di `range`, senza gli a capo finali (una riga `---` li include).
pub fn content_end(source: &str, range: Range<usize>) -> usize {
    let start = range.start;
    source.get(range).map_or(start, |content| start + content.trim_end_matches(['\n', '\r']).len())
}

/// C'e' una riga vuota in `source[from..to]`? La prima riga e' la fine del contenuto
/// precedente e l'ultima l'inizio del blocco: contano solo quelle in mezzo. Vale come
/// vuota anche una riga di soli spazi e `>` (la riga vuota dentro una citazione).
pub fn blank_line_between(source: &str, from: usize, to: usize) -> bool {
    let Some(between) = source.get(from.min(to)..to) else { return false };
    let mut inner = between.split('\n').skip(1).peekable();
    while let Some(line) = inner.next() {
        if inner.peek().is_some() && line.chars().all(|c| c.is_whitespace() || c == '>') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;
