//! I testi in italiano dei messaggi evento e dei titoli dei thread: brevi, i
//! dati completi sono nel campo `event` del messaggio.

use super::diff::TaskChange;
use super::snapshot::{ProposalSnap, TaskSnap};
use crate::board::{TaskKind, Verdict};

/// Task elencati nel messaggio di una nuova proposta; gli altri si contano.
const PROPOSAL_LIST_MAX: usize = 20;
/// Titolo di riserva di un thread se il titolo del task e' vuoto dopo la pulizia.
const FALLBACK_TITLE: &str = "Obiettivo";

pub const PROPOSAL_REJECTED: &str = "Proposta scartata.";
pub const THREAD_CONCLUDED: &str = "Tutti i task sono conclusi: thread chiuso.";

/// Titolo di un thread: il primo task, con "+N" se ce ne sono altri.
pub fn thread_title<'a>(mut titles: impl ExactSizeIterator<Item = &'a str>) -> String {
    let others = titles.len().saturating_sub(1);
    let first = titles.next().filter(|t| !t.is_empty()).unwrap_or(FALLBACK_TITLE);
    if others == 0 {
        first.to_string()
    } else {
        format!("{first} +{others}")
    }
}

pub fn proposal_created(p: &ProposalSnap) -> String {
    let mut lines = vec![format!("Nuova proposta: {} task", p.tasks.len())];
    lines.extend(p.tasks.iter().take(PROPOSAL_LIST_MAX).map(|t| format!("- {}", t.title)));
    let hidden = p.tasks.len().saturating_sub(PROPOSAL_LIST_MAX);
    if hidden > 0 {
        lines.push(format!("… e altri {hidden}"));
    }
    lines.join("\n")
}

pub fn proposal_accepted(task_count: usize) -> String {
    format!("Proposta accettata: {task_count} task in coda.")
}

pub fn task(change: TaskChange, task: &TaskSnap) -> String {
    let title = &task.title;
    match change {
        TaskChange::Created => format!("Task creato: {title}"),
        TaskChange::Started => format!("Task avviato: {title}"),
        TaskChange::Done => with_detail(done_headline(task), task.summary.as_deref()),
        TaskChange::Failed => {
            with_detail(format!("Task fallito: {title}"), task.note.as_deref().or(task.summary.as_deref()))
        }
        TaskChange::Attention => format!("Il task richiede attenzione: {title}"),
        TaskChange::Deleted => format!("Task eliminato: {title}"),
    }
}

fn done_headline(task: &TaskSnap) -> String {
    match (task.kind, task.verdict) {
        (TaskKind::Review, Some(verdict)) => {
            format!("Revisione completata: {} (esito: {})", task.title, verdict_label(verdict))
        }
        _ => format!("Task completato: {}", task.title),
    }
}

fn verdict_label(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Passed => "superata",
        Verdict::Issues => "problemi trovati",
    }
}

fn with_detail(headline: String, detail: Option<&str>) -> String {
    match detail {
        Some(d) => format!("{headline}\n{d}"),
        None => headline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::events::fixtures::{proposal, task};

    #[test]
    fn thread_titles_count_the_other_tasks() {
        assert_eq!(thread_title(["Login"].into_iter()), "Login");
        assert_eq!(thread_title(["Login", "API", "UI"].into_iter()), "Login +2");
        assert_eq!(thread_title(std::iter::empty()), FALLBACK_TITLE);
        assert_eq!(thread_title([""].into_iter()), FALLBACK_TITLE);
    }

    #[test]
    fn long_proposals_list_their_first_tasks_only() {
        let ids: Vec<String> = (0..PROPOSAL_LIST_MAX + 3).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let body = proposal_created(&proposal("p-1", &refs));
        assert!(body.starts_with(&format!("Nuova proposta: {} task\n- Task t0\n", PROPOSAL_LIST_MAX + 3)));
        assert!(body.ends_with("… e altri 3"));
        assert_eq!(body.lines().count(), PROPOSAL_LIST_MAX + 2);
    }

    #[test]
    fn task_texts() {
        let mut t = task("a", crate::board::TaskStatus::Done);
        assert_eq!(super::task(TaskChange::Done, &t), "Task completato: Task a");
        t.summary = Some("fatto".to_string());
        assert_eq!(super::task(TaskChange::Done, &t), "Task completato: Task a\nfatto");
        // Senza nota, il fallimento mostra il riassunto del report.
        assert_eq!(super::task(TaskChange::Failed, &t), "Task fallito: Task a\nfatto");
        (t.kind, t.verdict) = (TaskKind::Review, Some(Verdict::Passed));
        assert_eq!(super::task(TaskChange::Done, &t), "Revisione completata: Task a (esito: superata)\nfatto");
        assert_eq!(super::task(TaskChange::Deleted, &t), "Task eliminato: Task a");
    }
}
