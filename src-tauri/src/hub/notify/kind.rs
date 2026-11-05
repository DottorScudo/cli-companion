//! I tipi di notifica, con titolo, suono e significativita'.

use super::notice::HubNoticeSound;

/// Perche' si notifica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeKind {
    /// Un segretario ha risposto nel thread.
    Reply {
        bot: String,
    },
    /// Un worker ha una domanda.
    Question,
    /// Un messaggio di un mittente esterno (autore `external`) e' arrivato nel
    /// thread; `name` e' come si presenta (gia' ripulito).
    External {
        name: String,
    },
    TaskFailed,
    TaskAttention,
    /// La pianificazione e' finita senza proposta: servono risposte.
    PlanWithoutProposal,
    ProposalCreated,
    ThreadConcluded,
    ReviewDone,
    TaskDone,
}

/// Significativita' di un tipo, dalla minore alla maggiore (l'ordine delle
/// varianti): tra piu' candidati dello stesso thread vince la maggiore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    TaskDone,
    ReviewDone,
    ThreadConcluded,
    ProposalCreated,
    PlanWithoutProposal,
    TaskAttention,
    TaskFailed,
    Question,
    External,
    Reply,
}

impl NoticeKind {
    pub fn priority(&self) -> Priority {
        match self {
            Self::Reply { .. } => Priority::Reply,
            Self::Question => Priority::Question,
            Self::External { .. } => Priority::External,
            Self::TaskFailed => Priority::TaskFailed,
            Self::TaskAttention => Priority::TaskAttention,
            Self::PlanWithoutProposal => Priority::PlanWithoutProposal,
            Self::ProposalCreated => Priority::ProposalCreated,
            Self::ThreadConcluded => Priority::ThreadConcluded,
            Self::ReviewDone => Priority::ReviewDone,
            Self::TaskDone => Priority::TaskDone,
        }
    }

    /// Attenzione quando serve l'utente, ding per il resto.
    pub fn sound(&self) -> HubNoticeSound {
        match self {
            Self::Question | Self::TaskFailed | Self::TaskAttention | Self::PlanWithoutProposal => {
                HubNoticeSound::Attention
            }
            Self::Reply { .. }
            | Self::External { .. }
            | Self::ProposalCreated
            | Self::ThreadConcluded
            | Self::ReviewDone
            | Self::TaskDone => HubNoticeSound::Ding,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Reply { bot } => format!("@{bot} ti ha risposto"),
            Self::Question => "Un worker ha una domanda".to_string(),
            Self::External { name } => format!("Nuovo da {name}"),
            Self::TaskFailed => "Task fallito".to_string(),
            Self::TaskAttention => "Un task richiede attenzione".to_string(),
            Self::PlanWithoutProposal => "Pianificazione conclusa: servono tue risposte".to_string(),
            Self::ProposalCreated => "Nuova proposta di task".to_string(),
            Self::ThreadConcluded => "Thread concluso".to_string(),
            Self::ReviewDone => "Revisione finita".to_string(),
            Self::TaskDone => "Task finito".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_significance() -> Vec<NoticeKind> {
        vec![
            NoticeKind::Reply { bot: "ada".into() },
            NoticeKind::External { name: "Mario".into() },
            NoticeKind::Question,
            NoticeKind::TaskFailed,
            NoticeKind::TaskAttention,
            NoticeKind::PlanWithoutProposal,
            NoticeKind::ProposalCreated,
            NoticeKind::ThreadConcluded,
            NoticeKind::ReviewDone,
            NoticeKind::TaskDone,
        ]
    }

    #[test]
    fn priorities_follow_the_documented_order() {
        let kinds = by_significance();
        for pair in kinds.windows(2) {
            assert!(pair[0].priority() > pair[1].priority(), "{:?} > {:?}", pair[0], pair[1]);
        }
    }

    #[test]
    fn only_kinds_that_need_the_user_use_the_attention_sound() {
        let attention: Vec<bool> = by_significance().iter().map(|k| k.sound() == HubNoticeSound::Attention).collect();
        assert_eq!(attention, [false, false, true, true, true, true, false, false, false, false]);
    }

    #[test]
    fn titles_are_italian_and_a_reply_names_the_bot() {
        let titles: Vec<String> = by_significance().iter().map(NoticeKind::title).collect();
        assert_eq!(
            titles,
            [
                "@ada ti ha risposto",
                "Nuovo da Mario",
                "Un worker ha una domanda",
                "Task fallito",
                "Un task richiede attenzione",
                "Pianificazione conclusa: servono tue risposte",
                "Nuova proposta di task",
                "Thread concluso",
                "Revisione finita",
                "Task finito",
            ]
        );
    }
}
