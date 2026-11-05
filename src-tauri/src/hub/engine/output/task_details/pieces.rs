//! Pezzi di testi ed elenchi che stanno in un esito di tool (puro): la misura
//! e' quella della stringa JSON che il modello riceve, con le virgolette, le
//! barre e gli a capo che il JSON raddoppia. I testi si tagliano per
//! caratteri (mai in mezzo a uno), gli elenchi per voci intere.

use crate::hub::engine::turn::TOOL_RESULT_MAX;
use serde_json::{json, Map, Value};

/// Caratteri di `value` come il modello lo riceve (JSON serializzato).
pub fn size(value: &Value) -> usize {
    value.to_string().chars().count()
}

/// Un testo o un elenco, letto a pezzi.
#[derive(Debug, Clone, Copy)]
pub enum Content<'a> {
    Text(&'a str),
    Items(&'a [Value]),
}

/// Un pezzo di un `Content`: il valore, i caratteri che aggiunge al JSON del
/// valore vuoto e dove finisce (in caratteri o voci, escluso).
pub struct Piece {
    pub value: Value,
    pub used: usize,
    pub end: usize,
}

impl<'a> Content<'a> {
    /// Il testo o l'elenco in `value`; `None` per gli altri valori.
    pub fn of(value: &'a Value) -> Option<Self> {
        match value {
            Value::String(text) => Some(Self::Text(text)),
            Value::Array(items) => Some(Self::Items(items)),
            _ => None,
        }
    }

    /// Caratteri (testo) o voci (elenco).
    pub fn total(self) -> usize {
        match self {
            Self::Text(text) => text.chars().count(),
            Self::Items(items) => items.len(),
        }
    }

    /// Il valore vuoto dello stesso tipo.
    pub fn empty(self) -> Value {
        match self {
            Self::Text(_) => Value::from(""),
            Self::Items(_) => Value::Array(Vec::new()),
        }
    }

    /// La chiave del pezzo in una pagina di `task_details`.
    pub fn content_key(self) -> &'static str {
        match self {
            Self::Text(_) => "content",
            Self::Items(_) => "items",
        }
    }

    /// La chiave del totale (in una pagina e nelle voci di `more`).
    pub fn total_key(self) -> &'static str {
        match self {
            Self::Text(_) => "total_chars",
            Self::Items(_) => "total_items",
        }
    }

    /// L'unita' di `offset`, per gli errori.
    pub fn unit(self) -> &'static str {
        match self {
            Self::Text(_) => "characters",
            Self::Items(_) => "items",
        }
    }

    /// Il pezzo piu' lungo da `offset` che aggiunge al JSON del valore vuoto
    /// al massimo `room` caratteri.
    pub fn piece(self, offset: usize, room: usize) -> Piece {
        match self {
            Self::Text(text) => text_piece(text, offset, room),
            Self::Items(items) => items_piece(items, offset, room),
        }
    }

    /// I caratteri che il contenuto intero aggiunge al JSON del valore vuoto.
    fn cost(self) -> usize {
        self.piece(0, usize::MAX).used
    }

    /// Cosa resta da leggere dopo `end`: la voce di `more`.
    pub fn remaining(self, end: usize) -> Value {
        json!({ "next_offset": end, self.total_key(): self.total() })
    }
}

fn text_piece(text: &str, offset: usize, room: usize) -> Piece {
    let rest = text.char_indices().nth(offset).map_or("", |(i, _)| &text[i..]);
    let (mut used, mut chars, mut bytes) = (0, 0, 0);
    for c in rest.chars() {
        let cost = escaped_len(c);
        if used + cost > room {
            break;
        }
        used += cost;
        chars += 1;
        bytes += c.len_utf8();
    }
    Piece { value: Value::from(&rest[..bytes]), used, end: offset + chars }
}

fn items_piece(items: &[Value], offset: usize, room: usize) -> Piece {
    let mut used = 0;
    let mut taken = Vec::new();
    for item in items.iter().skip(offset) {
        let cost = size(item) + usize::from(!taken.is_empty());
        if used + cost > room {
            break;
        }
        used += cost;
        taken.push(item.clone());
    }
    Piece { end: offset + taken.len(), value: Value::Array(taken), used }
}

/// Caratteri di `c` dentro una stringa JSON: serde_json escapa solo le
/// virgolette, la barra rovesciata e i caratteri di controllo.
fn escaped_len(c: char) -> usize {
    if c >= ' ' && c != '"' && c != '\\' {
        1
    } else {
        size(&Value::from(c.to_string())) - 2
    }
}

/// Un campo di un esito che si puo' accorciare: il nome per `more` e il suo JSON pointer.
pub struct Pageable {
    pub field: String,
    pub pointer: String,
}

/// Un campo da accorciare, tolto dall'esito, con quanto costerebbe intero.
struct Pending<'p> {
    pageable: &'p Pageable,
    full: Value,
    cost: usize,
}

/// Accorcia i campi `pageables` dell'oggetto `out` solo se l'esito non sta in
/// `TOOL_RESULT_MAX`: ognuno tiene il suo primo pezzo e `more` dice, per ogni
/// campo incompleto, da dove riprendere e quanto e' lungo. Lo spazio si divide
/// in parti uguali partendo dai campi che costano meno, cosi' quelli che stanno
/// nella loro parte restano interi e lasciano il resto agli altri; il resto
/// dell'esito (stato, commit, id) non si tocca mai.
pub fn fit(out: &mut Value, pageables: &[Pageable]) {
    if size(out) <= TOOL_RESULT_MAX {
        return;
    }
    let mut pending: Vec<Pending> = pageables.iter().filter_map(|p| take_out(out, p)).collect();
    pending.sort_by_key(|p| p.cost);
    // Ogni voce parte dal segnaposto piu' lungo possibile (`next_offset` uguale
    // al totale): alla fine si accorcia o sparisce, e l'esito resta nel limite.
    let mut more: Map<String, Value> = pending
        .iter()
        .filter_map(|p| Some((p.pageable.field.clone(), Content::of(&p.full).map(|c| c.remaining(c.total()))?)))
        .collect();
    out["more"] = Value::Object(more.clone());
    let mut room = TOOL_RESULT_MAX.saturating_sub(size(out));
    let count = pending.len();
    for (done, p) in pending.iter().enumerate() {
        let Some(content) = Content::of(&p.full) else {
            continue;
        };
        let piece = content.piece(0, room / (count - done));
        room -= piece.used;
        if let Some(slot) = out.pointer_mut(&p.pageable.pointer) {
            *slot = piece.value;
        }
        if piece.end == content.total() {
            more.remove(&p.pageable.field);
        } else {
            more.insert(p.pageable.field.clone(), content.remaining(piece.end));
        }
    }
    if let Some(fields) = out.as_object_mut() {
        if more.is_empty() {
            fields.remove("more");
        } else {
            fields.insert("more".to_string(), Value::Object(more));
        }
    }
}

/// Toglie da `out` il campo `p` (se e' un testo o un elenco non vuoto),
/// lasciando al suo posto il valore vuoto.
fn take_out<'p>(out: &mut Value, p: &'p Pageable) -> Option<Pending<'p>> {
    let slot = out.pointer_mut(&p.pointer)?;
    let content = Content::of(slot).filter(|c| c.total() > 0)?;
    let (empty, cost) = (content.empty(), content.cost());
    Some(Pending { pageable: p, full: std::mem::replace(slot, empty), cost })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pageable(field: &str) -> Pageable {
        Pageable { field: field.into(), pointer: format!("/{field}") }
    }

    #[test]
    fn a_text_piece_counts_what_json_doubles_and_never_splits_a_character() {
        let value = Value::from("a\"è\n😀b");
        let content = Content::of(&value).unwrap();
        let piece = content.piece(0, 4);
        assert_eq!((piece.value, piece.used, piece.end), (Value::from("a\"è"), 4, 3));
        let piece = content.piece(3, 3);
        assert_eq!((piece.value, piece.used, piece.end), (Value::from("\n😀"), 3, 5));
        assert_eq!(content.cost(), size(&value) - 2);
        assert_eq!(content.piece(6, 10).end, 6);
    }

    #[test]
    fn control_characters_cost_what_serde_json_writes() {
        let value = Value::from("\u{1}x\u{7f}\\");
        assert_eq!(Content::of(&value).unwrap().cost(), size(&value) - 2);
    }

    #[test]
    fn an_items_piece_takes_whole_items_and_their_commas() {
        let value = json!(["ab", "cd", "ef"]);
        let content = Content::of(&value).unwrap();
        let piece = content.piece(1, 9);
        assert_eq!((piece.value, piece.used, piece.end), (json!(["cd", "ef"]), 9, 3));
        assert_eq!(content.piece(0, 3).end, 0);
        assert_eq!(content.cost(), size(&value) - 2);
    }

    #[test]
    fn what_fits_is_left_alone() {
        let mut out = json!({ "a": "short", "b": ["x"] });
        let before = out.clone();
        fit(&mut out, &[pageable("a"), pageable("b")]);
        assert_eq!(out, before);
    }

    #[test]
    fn the_short_fields_stay_whole_and_the_long_one_says_where_to_resume() {
        let long = "y".repeat(TOOL_RESULT_MAX * 2);
        let mut out = json!({ "id": "t-1", "a": "short", "b": long, "c": [], "d": 4 });
        fit(&mut out, &[pageable("a"), pageable("b"), pageable("c"), pageable("d"), pageable("missing")]);
        assert!(size(&out) <= TOOL_RESULT_MAX, "{}", size(&out));
        assert_eq!((out["id"].as_str(), out["a"].as_str(), out["d"].as_i64()), (Some("t-1"), Some("short"), Some(4)));
        let kept = out["b"].as_str().unwrap().chars().count();
        assert_eq!(out["more"], json!({ "b": { "next_offset": kept, "total_chars": long.len() } }));
        assert!(kept > TOOL_RESULT_MAX - 200, "{kept}");
    }

    #[test]
    fn the_space_left_by_a_short_field_goes_to_the_long_ones() {
        let long = "y".repeat(TOOL_RESULT_MAX);
        let mut out = json!({ "a": long, "b": long, "c": "z".repeat(10) });
        fit(&mut out, &[pageable("a"), pageable("b"), pageable("c")]);
        assert!(size(&out) <= TOOL_RESULT_MAX);
        let (a, b) = (out["a"].as_str().unwrap().len(), out["b"].as_str().unwrap().len());
        assert!(a.abs_diff(b) <= 1 && a > TOOL_RESULT_MAX / 2 - 100, "{a} {b}");
        assert_eq!(out["c"], "z".repeat(10));
    }
}
