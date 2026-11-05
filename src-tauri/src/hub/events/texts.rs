//! I testi in italiano dei messaggi evento e dei titoli dei thread: una riga
//! (tipo di evento e titolo del task), i dati completi sono nel campo `event`
//! del messaggio. Fanno eccezione il piano senza proposta, che porta le domande
//! per l'utente, la domanda di un worker (testo, opzioni e come rispondere) e
//! il fallimento, che aggiunge la prima riga del motivo.

use super::diff::TaskChange;
use super::snapshot::{ProposalSnap, TaskSnap};
use crate::board::{TaskKind, Verdict};
use crate::validate::clip_with_ellipsis;

/// Task elencati nel messaggio di una nuova proposta; gli altri si contano.
const PROPOSAL_LIST_MAX: usize = 20;
/// Titolo di riserva di un thread se il titolo del task e' vuoto dopo la pulizia.
const FALLBACK_TITLE: &str = "Obiettivo";
/// Caratteri del motivo di un fallimento riportati nel messaggio.
const FAILURE_REASON_MAX: usize = 120;
/// Separatore tra il titolo di un task fallito e il suo motivo.
const REASON_SEPARATOR: &str = " — ";

/// Separatore delle risposte suggerite di una domanda.
const OPTIONS_SEPARATOR: &str = " · ";
/// Come si risponde a una domanda dal thread.
const QUESTION_HINT: &str = "Usa «Rispondi» su questo messaggio: la tua risposta arriva al worker così com'è.";

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
        TaskChange::Done => done_headline(task),
        TaskChange::Failed => failed_headline(task),
        TaskChange::Attention => format!("Il task richiede attenzione: {title}"),
        TaskChange::Question => question_asked(task),
        TaskChange::QuestionAnswered => format!("Domanda risposta: {title}"),
        TaskChange::Deleted => format!("Task eliminato: {title}"),
    }
}

/// La domanda di un worker o del pianificatore: chi la fa, il testo, le
/// risposte suggerite e come rispondere (nel thread non si vede altrove).
fn question_asked(task: &TaskSnap) -> String {
    let who = if task.kind == TaskKind::Plan { "Domanda del pianificatore" } else { "Domanda del worker" };
    let mut lines = vec![format!("{who}: {}", task.title)];
    if let Some(question) = &task.question {
        lines.push(question.text.clone());
        if !question.options.is_empty() {
            lines.push(format!("Opzioni: {}", question.options.join(OPTIONS_SEPARATOR)));
        }
    }
    lines.push(QUESTION_HINT.to_string());
    lines.join("\n")
}

/// Un task `plan` concluso senza proposta: il riassunto porta le domande del
/// pianificatore per l'utente, che nel thread non si vedono altrove.
pub fn plan_without_proposal(task: &TaskSnap) -> String {
    let headline = format!("Pianificazione conclusa senza proposta: {}", task.title);
    match task.summary.as_deref() {
        Some(questions) => format!("{headline}\n{questions}"),
        None => headline,
    }
}

fn done_headline(task: &TaskSnap) -> String {
    match (task.kind, task.verdict) {
        (TaskKind::Review, Some(verdict)) => {
            format!("Revisione finita: {} (esito: {})", task.title, verdict_label(verdict))
        }
        _ => format!("Task finito: {}", task.title),
    }
}

fn failed_headline(task: &TaskSnap) -> String {
    let headline = format!("Task fallito: {}", task.title);
    match failure_reason(task) {
        Some(reason) => format!("{headline}{REASON_SEPARATOR}{reason}"),
        None => headline,
    }
}

/// La prima riga non vuota della nota (o, se manca, del riassunto), tagliata.
fn failure_reason(task: &TaskSnap) -> Option<String> {
    [task.note.as_deref(), task.summary.as_deref()]
        .into_iter()
        .flatten()
        .find_map(|text| text.lines().map(str::trim).find(|line| !line.is_empty()))
        .map(|line| clip_with_ellipsis(line, FAILURE_REASON_MAX))
}

fn verdict_label(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Passed => "superata",
        Verdict::Issues => "problemi trovati",
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
    fn task_texts_are_one_line_without_the_summary() {
        let mut t = task("a", crate::board::TaskStatus::Done);
        t.summary = Some("fatto\naltro".to_string());
        assert_eq!(super::task(TaskChange::Created, &t), "Task creato: Task a");
        assert_eq!(super::task(TaskChange::Started, &t), "Task avviato: Task a");
        assert_eq!(super::task(TaskChange::Done, &t), "Task finito: Task a");
        assert_eq!(super::task(TaskChange::Attention, &t), "Il task richiede attenzione: Task a");
        (t.kind, t.verdict) = (TaskKind::Review, Some(Verdict::Passed));
        assert_eq!(super::task(TaskChange::Done, &t), "Revisione finita: Task a (esito: superata)");
        t.verdict = Some(Verdict::Issues);
        assert_eq!(super::task(TaskChange::Done, &t), "Revisione finita: Task a (esito: problemi trovati)");
        assert_eq!(super::task(TaskChange::Deleted, &t), "Task eliminato: Task a");
    }

    #[test]
    fn a_failure_reports_the_first_line_of_its_reason() {
        let mut t = task("a", crate::board::TaskStatus::Failed);
        assert_eq!(super::task(TaskChange::Failed, &t), "Task fallito: Task a");
        // Senza nota, il motivo e' la prima riga non vuota del riassunto.
        t.summary = Some("\n  \nprima riga\nseconda".to_string());
        assert_eq!(super::task(TaskChange::Failed, &t), "Task fallito: Task a — prima riga");
        t.note = Some("worker interrotto\ndettagli".to_string());
        assert_eq!(super::task(TaskChange::Failed, &t), "Task fallito: Task a — worker interrotto");
        t.note = Some("   ".to_string());
        assert_eq!(super::task(TaskChange::Failed, &t), "Task fallito: Task a — prima riga");
    }

    #[test]
    fn a_long_failure_reason_is_cut_with_an_ellipsis() {
        let mut t = task("a", crate::board::TaskStatus::Failed);
        t.note = Some("è".repeat(FAILURE_REASON_MAX + 5));
        let body = super::task(TaskChange::Failed, &t);
        let reason = body.strip_prefix("Task fallito: Task a — ").unwrap();
        assert_eq!(reason, format!("{}…", "è".repeat(FAILURE_REASON_MAX)));
        t.note = Some("è".repeat(FAILURE_REASON_MAX));
        assert!(!super::task(TaskChange::Failed, &t).ends_with('…'));
    }

    #[test]
    fn a_plan_without_proposal_keeps_the_questions() {
        let mut t = task("a", crate::board::TaskStatus::Done);
        assert_eq!(plan_without_proposal(&t), "Pianificazione conclusa senza proposta: Task a");
        t.summary = Some("Questions for the user:\n1. Quale database?".to_string());
        assert_eq!(
            plan_without_proposal(&t),
            "Pianificazione conclusa senza proposta: Task a\nQuestions for the user:\n1. Quale database?"
        );
    }
}
