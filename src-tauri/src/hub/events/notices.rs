//! Quali eventi della board producono una notifica di sistema (puro). Gli
//! eventi senza notifica (task creato o avviato, task eliminato, domanda
//! risposta, proposta accettata o scartata) non compaiono qui.

use super::diff::TaskChange;
use crate::board::TaskKind;
use crate::hub::notify::{NoticeKind, PlanEnd};

/// Una notifica da dare per un messaggio appena scritto in un thread: `text` e'
/// il corpo del messaggio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventNotice {
    pub thread_id: String,
    pub message_id: String,
    pub kind: NoticeKind,
    pub text: String,
}

/// La notifica di un evento di un task. `unanswered_plan`: il task `plan` e'
/// finito senza una proposta, con le domande per l'utente o con la risposta
/// alla sua domanda.
pub fn for_task(change: TaskChange, kind: TaskKind, unanswered_plan: Option<PlanEnd>) -> Option<NoticeKind> {
    match (change, unanswered_plan) {
        (TaskChange::Done, Some(end)) => Some(NoticeKind::PlanWithoutProposal(end)),
        (TaskChange::Done, None) if kind == TaskKind::Review => Some(NoticeKind::ReviewDone),
        (TaskChange::Done, None) => Some(NoticeKind::TaskDone),
        (TaskChange::Failed, _) => Some(NoticeKind::TaskFailed),
        (TaskChange::Attention, _) => Some(NoticeKind::TaskAttention),
        (TaskChange::Question, _) => Some(NoticeKind::Question),
        (TaskChange::Created | TaskChange::Started | TaskChange::QuestionAnswered | TaskChange::Deleted, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_task_depends_on_its_kind() {
        assert_eq!(for_task(TaskChange::Done, TaskKind::Work, None), Some(NoticeKind::TaskDone));
        assert_eq!(for_task(TaskChange::Done, TaskKind::Review, None), Some(NoticeKind::ReviewDone));
        assert_eq!(for_task(TaskChange::Done, TaskKind::Plan, None), Some(NoticeKind::TaskDone));
    }

    #[test]
    fn a_plan_without_a_proposal_wins_over_done_and_keeps_how_it_ended() {
        for end in [PlanEnd::Questions, PlanEnd::Answer] {
            assert_eq!(
                for_task(TaskChange::Done, TaskKind::Plan, Some(end)),
                Some(NoticeKind::PlanWithoutProposal(end))
            );
        }
    }

    #[test]
    fn problems_and_questions_notify() {
        assert_eq!(for_task(TaskChange::Failed, TaskKind::Work, None), Some(NoticeKind::TaskFailed));
        assert_eq!(for_task(TaskChange::Attention, TaskKind::Work, None), Some(NoticeKind::TaskAttention));
        assert_eq!(for_task(TaskChange::Question, TaskKind::Work, None), Some(NoticeKind::Question));
    }

    #[test]
    fn the_other_changes_stay_silent() {
        for change in [TaskChange::Created, TaskChange::Started, TaskChange::QuestionAnswered, TaskChange::Deleted] {
            assert_eq!(for_task(change, TaskKind::Work, None), None, "{change:?}");
        }
    }
}
