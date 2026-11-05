//! La ricerca di `board_status` (pura, senza app): filtra, cerca, ordina dal
//! piu' recente e prende una pagina dei task di una board. L'output per il
//! modello e' in `output`; qui si decide solo quali task e in che ordine.

use crate::board::{TaskKind, TaskStatus, TaskView};
use std::cmp::Reverse;

/// Caratteri massimi del testo cercato.
pub const QUERY_MAX: usize = 200;
/// Task di una pagina quando il modello non chiede un `limit`.
pub const PAGE_DEFAULT: usize = 20;
/// Task di una pagina al massimo.
pub const PAGE_MAX: usize = 40;

/// Cosa chiede il modello. I filtri si combinano in AND; un filtro assente non
/// restringe nulla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardQuery {
    /// Stati ammessi; vuoto = tutti.
    pub statuses: Vec<TaskStatus>,
    /// Solo i task che aspettano l'utente (`needs_user`).
    pub attention: bool,
    pub kind: Option<TaskKind>,
    /// Testo da cercare (senza distinguere maiuscole e minuscole), gia' ripulito;
    /// `None` = nessuna ricerca.
    pub text: Option<String>,
    /// Solo i task aggiornati da questo istante in poi (ms UTC, incluso).
    pub updated_since: Option<i64>,
    /// Solo i task aggiornati prima di questo istante (ms UTC, escluso).
    pub updated_before: Option<i64>,
    pub offset: usize,
    /// Task della pagina, tra 1 e `PAGE_MAX`.
    pub limit: usize,
}

impl Default for BoardQuery {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            attention: false,
            kind: None,
            text: None,
            updated_since: None,
            updated_before: None,
            offset: 0,
            limit: PAGE_DEFAULT,
        }
    }
}

/// Una pagina di risultati: i task dal piu' recente.
#[derive(Debug, PartialEq, Eq)]
pub struct Page<'a> {
    /// I task che soddisfano i filtri, in tutte le pagine.
    pub total: usize,
    pub tasks: Vec<&'a TaskView>,
    /// Dove riprendere per la pagina dopo; assente sull'ultima.
    pub next_offset: Option<usize>,
}

/// I task della board che soddisfano `query`, dal piu' recente (creato per ultimo;
/// a parita' di ora, aggiunto per ultimo alla board), e la pagina chiesta.
pub fn select<'a>(tasks: &'a [TaskView], query: &BoardQuery) -> Page<'a> {
    let needle = query.text.as_deref().map(str::to_lowercase);
    let mut found: Vec<&TaskView> = tasks.iter().rev().filter(|v| matches(v, query, needle.as_deref())).collect();
    found.sort_by_key(|v| Reverse(v.task.created_at));
    let total = found.len();
    let end = query.offset.saturating_add(query.limit).min(total);
    let tasks = found.get(query.offset..end).unwrap_or_default().to_vec();
    Page { total, tasks, next_offset: (end < total).then_some(end) }
}

fn matches(v: &TaskView, query: &BoardQuery, needle: Option<&str>) -> bool {
    (query.statuses.is_empty() || query.statuses.contains(&v.task.status()))
        && (!query.attention || needs_user(v))
        && query.kind.is_none_or(|k| v.task.kind == k)
        && query.updated_since.is_none_or(|since| v.task.updated_at >= since)
        && query.updated_before.is_none_or(|before| v.task.updated_at < before)
        && needle.is_none_or(|n| mentions(v, n))
}

/// Il task richiede l'utente: il worker aspetta lui (`attention`), ha una
/// domanda senza risposta o l'agente non e' pronto e il task non puo' partire.
pub fn needs_user(v: &TaskView) -> bool {
    v.attention || v.question.is_some() || v.agent_unavailable.is_some()
}

/// `needle` (gia' in minuscolo) compare nell'id, nel titolo, nel riassunto del
/// report, nella nota o nel testo della specifica.
fn mentions(v: &TaskView, needle: &str) -> bool {
    let t = &v.task;
    [Some(t.id.as_str()), Some(&t.title), t.report().map(|r| r.summary.as_str()), t.note(), Some(&t.task)]
        .into_iter()
        .flatten()
        .any(|text| text.to_lowercase().contains(needle))
}

#[cfg(test)]
mod tests;
