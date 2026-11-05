//! Il raggruppamento per thread, puro: i candidati di uno stesso thread nella
//! stessa finestra diventano una notifica sola, la piu' significativa.

use super::kind::Priority;
use super::notice::HubNotice;
use crate::hub::channel::Channel;
use std::collections::HashMap;

/// Il thread a cui appartiene un candidato.
pub type ThreadKey = (Channel, String);

/// Una notifica in attesa, con la sua significativita'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub priority: Priority,
    pub notice: HubNotice,
}

/// I candidati in attesa, uno per thread.
#[derive(Debug, Default)]
pub struct Pending {
    slots: HashMap<ThreadKey, Candidate>,
}

impl Pending {
    /// Offre un candidato. Ritorna `true` se e' il primo del thread nella
    /// finestra (chi lo offre deve avviarla); altrimenti resta il piu'
    /// significativo e, a parita', l'ultimo arrivato.
    pub fn offer(&mut self, key: ThreadKey, candidate: Candidate) -> bool {
        match self.slots.get_mut(&key) {
            Some(kept) => {
                if candidate.priority >= kept.priority {
                    *kept = candidate;
                }
                false
            }
            None => {
                self.slots.insert(key, candidate);
                true
            }
        }
    }

    /// Chiude la finestra del thread e ne consegna la notifica.
    pub fn take(&mut self, key: &ThreadKey) -> Option<HubNotice> {
        self.slots.remove(key).map(|c| c.notice)
    }

    /// Butta tutto (HUB spento).
    pub fn clear(&mut self) {
        self.slots.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::notice::HubNoticeSound;
    use super::*;

    fn key(thread: &str) -> ThreadKey {
        (Channel::General, thread.to_string())
    }

    fn candidate(thread: &str, priority: Priority, title: &str) -> Candidate {
        let notice = HubNotice {
            channel: Channel::General,
            thread_id: thread.to_string(),
            message_id: "msg-1".to_string(),
            title: title.to_string(),
            body: String::new(),
            sound: HubNoticeSound::Ding,
        };
        Candidate { priority, notice }
    }

    #[test]
    fn the_first_candidate_opens_the_window() {
        let mut p = Pending::default();
        assert!(p.offer(key("a"), candidate("a", Priority::TaskDone, "1")));
        assert!(!p.offer(key("a"), candidate("a", Priority::TaskDone, "2")));
    }

    #[test]
    fn threads_are_independent() {
        let mut p = Pending::default();
        assert!(p.offer(key("a"), candidate("a", Priority::Reply, "a")));
        assert!(p.offer(key("b"), candidate("b", Priority::TaskDone, "b")));
        assert_eq!(p.take(&key("b")).unwrap().title, "b");
        assert_eq!(p.take(&key("a")).unwrap().title, "a");
    }

    #[test]
    fn the_reply_beats_the_event_in_either_order() {
        let mut p = Pending::default();
        p.offer(key("a"), candidate("a", Priority::TaskFailed, "evento"));
        p.offer(key("a"), candidate("a", Priority::Reply, "risposta"));
        assert_eq!(p.take(&key("a")).unwrap().title, "risposta");
        p.offer(key("a"), candidate("a", Priority::Reply, "risposta"));
        p.offer(key("a"), candidate("a", Priority::TaskFailed, "evento"));
        assert_eq!(p.take(&key("a")).unwrap().title, "risposta");
    }

    #[test]
    fn a_tie_keeps_the_latest() {
        let mut p = Pending::default();
        p.offer(key("a"), candidate("a", Priority::Question, "prima"));
        p.offer(key("a"), candidate("a", Priority::Question, "dopo"));
        assert_eq!(p.take(&key("a")).unwrap().title, "dopo");
    }

    #[test]
    fn take_empties_the_slot_and_reopens_the_window() {
        let mut p = Pending::default();
        p.offer(key("a"), candidate("a", Priority::Reply, "x"));
        assert!(p.take(&key("a")).is_some());
        assert!(p.take(&key("a")).is_none());
        assert!(p.offer(key("a"), candidate("a", Priority::TaskDone, "y")));
    }

    #[test]
    fn clear_drops_everything() {
        let mut p = Pending::default();
        p.offer(key("a"), candidate("a", Priority::Reply, "x"));
        p.clear();
        assert!(p.take(&key("a")).is_none());
    }
}
