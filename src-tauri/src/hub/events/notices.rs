//! Quali eventi della board producono una notifica di sistema (puro). Gli
//! eventi senza notifica (task creato o avviato, task eliminato, domanda
//! risposta, proposta accettata o scartata) non compaiono qui.

use super::diff::TaskChange;
use crate::board::TaskKind;
use crate::hub::notify::NoticeKind;

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
/// finito senza una proposta (i suoi dubbi sono per l'utente).
pub fn for_task(change: TaskChange, kind: TaskKind, unanswered_plan: bool) -> Option<NoticeKind> {
    match change {
        TaskChange::Done if unanswered_plan => Some(NoticeKind::PlanWithoutProposal),
        TaskChange::Done if kind == TaskKind::Review => Some(NoticeKind::ReviewDone),
        TaskChange::Done => Some(NoticeKind::TaskDone),
        TaskChange::Failed => Some(NoticeKind::TaskFailed),
        TaskChange::Attention => Some(NoticeKind::TaskAttention),
        TaskChange::Question => Some(NoticeKind::Question),
        TaskChange::Created | TaskChange::Started | TaskChange::QuestionAnswered | TaskChange::Deleted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_task_depends_on_its_kind() {
        assert_eq!(for_task(TaskChange::Done, TaskKind::Work, false), Some(NoticeKind::TaskDone));
        assert_eq!(for_task(TaskChange::Done, TaskKind::Review, false), Some(NoticeKind::ReviewDone));
        assert_eq!(for_task(TaskChange::Done, TaskKind::Plan, false), Some(NoticeKind::TaskDone));
    }

    #[test]
    fn a_plan_without_a_proposal_wins_over_done() {
        assert_eq!(for_task(TaskChange::Done, TaskKind::Plan, true), Some(NoticeKind::PlanWithoutProposal));
    }

    #[test]
    fn problems_and_questions_notify() {
        assert_eq!(for_task(TaskChange::Failed, TaskKind::Work, false), Some(NoticeKind::TaskFailed));
        assert_eq!(for_task(TaskChange::Attention, TaskKind::Work, false), Some(NoticeKind::TaskAttention));
        assert_eq!(for_task(TaskChange::Question, TaskKind::Work, false), Some(NoticeKind::Question));
    }

    #[test]
    fn the_other_changes_stay_silent() {
        for change in [TaskChange::Created, TaskChange::Started, TaskChange::QuestionAnswered, TaskChange::Deleted] {
            assert_eq!(for_task(change, TaskKind::Work, false), None, "{change:?}");
        }
    }
}
