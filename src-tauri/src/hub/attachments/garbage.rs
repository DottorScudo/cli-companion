//! Quali file degli allegati cancellare dopo un salvataggio: quelli dei
//! messaggi che c'erano prima e non ci sono piu' (potati da `MESSAGES_MAX`) e
//! dei thread tolti (thread chiusi potati). Vale per ogni modifica, qualunque
//! operazione l'abbia fatta. Puro.

use crate::hub::model::Thread;
use crate::hub::threads::ChannelThreads;
use std::collections::{HashMap, HashSet};

/// I file da cancellare in un canale.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Garbage {
    /// Thread spariti che avevano allegati: via la loro cartella intera.
    pub threads: Vec<String>,
    /// `(thread_id, attachment_id)` dei messaggi spariti da thread che restano.
    pub attachments: Vec<(String, String)>,
}

impl Garbage {
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty() && self.attachments.is_empty()
    }
}

/// Gli allegati di `before` che `after` non ha piu'. Solo i thread con allegati
/// costano qualcosa: gli altri non hanno file.
pub fn garbage(before: &ChannelThreads, after: &ChannelThreads) -> Garbage {
    let after_by_id: HashMap<&str, &Thread> = after.threads.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut out = Garbage::default();
    for thread in before.threads.iter().filter(|t| has_attachments(t)) {
        let Some(kept) = after_by_id.get(thread.id.as_str()) else {
            out.threads.push(thread.id.clone());
            continue;
        };
        let still: HashSet<&str> = attachment_ids(kept).collect();
        out.attachments.extend(
            attachment_ids(thread).filter(|id| !still.contains(id)).map(|id| (thread.id.clone(), id.to_string())),
        );
    }
    out
}

fn has_attachments(thread: &Thread) -> bool {
    thread.messages.iter().any(|m| !m.attachments.is_empty())
}

fn attachment_ids(thread: &Thread) -> impl Iterator<Item = &str> {
    thread.messages.iter().flat_map(|m| m.attachments.iter().map(|a| a.id.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::channel::Channel;
    use crate::hub::model::{Attachment, Author, Message, MessageKind, ThreadState};

    fn message(id: &str, attachments: &[&str]) -> Message {
        Message {
            id: id.to_string(),
            author: Author::User,
            kind: MessageKind::Text,
            body: String::new(),
            reply_to: None,
            event: None,
            origin: None,
            attachments: attachments
                .iter()
                .map(|a| Attachment {
                    id: a.to_string(),
                    name: "f.txt".to_string(),
                    mime: "text/plain".to_string(),
                    size: 1,
                })
                .collect(),
            created_at: 0,
        }
    }

    fn thread(id: &str, messages: Vec<Message>) -> Thread {
        Thread {
            id: id.to_string(),
            channel: Channel::General,
            title: "t".to_string(),
            state: ThreadState::Open,
            task_ids: Vec::new(),
            proposal_id: None,
            owner: None,
            created_at: 0,
            updated_at: 0,
            summary: None,
            messages,
        }
    }

    fn list(threads: Vec<Thread>) -> ChannelThreads {
        ChannelThreads { threads }
    }

    #[test]
    fn nothing_removed_means_nothing_to_delete() {
        let before = list(vec![thread("t1", vec![message("m1", &["a1"])])]);
        let mut after = before.clone();
        after.threads[0].messages.push(message("m2", &["a2"]));
        assert!(garbage(&before, &after).is_empty());
        assert!(garbage(&list(vec![]), &after).is_empty());
    }

    #[test]
    fn pruned_messages_lose_their_attachments() {
        let before = list(vec![
            thread("t1", vec![message("m1", &["a1", "a2"]), message("m2", &["a3"]), message("m3", &[])]),
            thread("t2", vec![message("m4", &["a4"])]),
        ]);
        let mut after = before.clone();
        after.threads[0].messages.remove(0);
        let out = garbage(&before, &after);
        assert_eq!(out.attachments, vec![("t1".to_string(), "a1".to_string()), ("t1".to_string(), "a2".to_string())]);
        assert!(out.threads.is_empty());
    }

    #[test]
    fn removed_threads_lose_their_folder_only_when_they_had_attachments() {
        let before = list(vec![
            thread("t1", vec![message("m1", &["a1"])]),
            thread("t2", vec![message("m2", &[])]),
            thread("t3", vec![message("m3", &["a3"])]),
        ]);
        let after = list(vec![before.threads[2].clone()]);
        let out = garbage(&before, &after);
        assert_eq!(out.threads, vec!["t1".to_string()]);
        assert!(out.attachments.is_empty());
    }
}
