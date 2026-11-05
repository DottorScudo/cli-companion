//! Se un thread aspetta l'utente, calcolato dai suoi eventi (mai salvato: non
//! puo' divergere dai messaggi). Per ogni soggetto (task o proposta) conta solo
//! l'ultimo evento: un task fallito e poi ripartito non aspetta piu' nessuno,
//! una proposta accettata o scartata nemmeno.

use super::model::{HubEventKind, Message, Subject};
use std::collections::HashMap;

pub fn waiting_user(messages: &[Message]) -> bool {
    let mut last: HashMap<Subject<'_>, HubEventKind> = HashMap::new();
    for event in messages.iter().filter_map(|m| m.event.as_ref()) {
        if let Some(subject) = event.subject() {
            last.insert(subject, event.kind);
        }
    }
    last.values().any(|kind| kind.waits_for_user())
}

/// L'ultimo evento del soggetto aspetta ancora l'utente (per esempio una
/// proposta non ancora decisa).
pub fn subject_waiting(messages: &[Message], subject: Subject<'_>) -> bool {
    messages
        .iter()
        .rev()
        .filter_map(|m| m.event.as_ref())
        .find(|e| e.subject() == Some(subject))
        .is_some_and(|e| e.kind.waits_for_user())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::model::{Author, HubEvent, MessageKind};

    fn event(kind: HubEventKind, task: Option<&str>, proposal: Option<&str>) -> Message {
        Message {
            id: format!("m-{kind:?}"),
            author: Author::System,
            kind: MessageKind::Event,
            body: "evento".to_string(),
            reply_to: None,
            event: Some(HubEvent {
                task_id: task.map(str::to_string),
                proposal_id: proposal.map(str::to_string),
                ..HubEvent::new(kind)
            }),
            attachments: Vec::new(),
            origin: None,
            created_at: 0,
        }
    }

    fn text() -> Message {
        Message {
            id: "m-text".to_string(),
            author: Author::User,
            kind: MessageKind::Text,
            body: "ciao".to_string(),
            reply_to: None,
            event: None,
            attachments: Vec::new(),
            origin: None,
            created_at: 0,
        }
    }

    #[test]
    fn no_events_means_nothing_to_wait_for() {
        assert!(!waiting_user(&[]));
        assert!(!waiting_user(&[text()]));
    }

    #[test]
    fn failed_attention_and_pending_proposals_wait() {
        assert!(waiting_user(&[event(HubEventKind::TaskFailed, Some("a"), None)]));
        assert!(waiting_user(&[event(HubEventKind::TaskAttention, Some("a"), None)]));
        assert!(waiting_user(&[event(HubEventKind::ProposalCreated, None, Some("p-1"))]));
        assert!(waiting_user(&[event(HubEventKind::TaskQuestion, Some("a"), None)]));
        let answered = [
            event(HubEventKind::TaskQuestion, Some("a"), None),
            event(HubEventKind::TaskQuestionAnswered, Some("a"), None),
        ];
        assert!(!waiting_user(&answered), "una domanda risposta non aspetta piu'");
        assert!(!waiting_user(&[event(HubEventKind::TaskDone, Some("a"), None)]));
    }

    #[test]
    fn the_last_event_of_each_subject_wins() {
        let retried =
            [event(HubEventKind::TaskFailed, Some("a"), None), event(HubEventKind::TaskStarted, Some("a"), None)];
        assert!(!waiting_user(&retried));
        let decided = [
            event(HubEventKind::ProposalCreated, None, Some("p-1")),
            event(HubEventKind::ProposalAccepted, None, Some("p-1")),
        ];
        assert!(!waiting_user(&decided));
        let other_still_failed =
            [event(HubEventKind::TaskFailed, Some("a"), None), event(HubEventKind::TaskDone, Some("b"), None)];
        assert!(waiting_user(&other_still_failed));
    }

    #[test]
    fn a_single_subject_waits_until_its_last_event_says_otherwise() {
        let p1 = Subject::Proposal("p-1");
        let pending = [event(HubEventKind::ProposalCreated, Some("plan"), Some("p-1"))];
        assert!(subject_waiting(&pending, p1));
        assert!(!subject_waiting(&pending, Subject::Proposal("p-2")));
        assert!(!subject_waiting(&pending, Subject::Task("plan")), "il task_id di una proposta non e' il soggetto");
        let decided = [pending[0].clone(), event(HubEventKind::ProposalRejected, None, Some("p-1"))];
        assert!(!subject_waiting(&decided, p1));
        assert!(!subject_waiting(&[], p1));
    }

    #[test]
    fn events_without_their_subject_are_ignored() {
        assert!(!waiting_user(&[event(HubEventKind::TaskFailed, None, Some("p-1"))]));
        assert!(!waiting_user(&[event(HubEventKind::ProposalCreated, Some("a"), None)]));
    }
}
