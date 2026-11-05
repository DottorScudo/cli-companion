//! I candidati di ripiego per un messaggio dell'utente senza menzioni di
//! segretari esistenti (puro): li ricava il chiamante dal thread salvato, cosi'
//! `Sessions::route` decide senza leggere i thread.

use crate::hub::model::{Author, Message, Thread};

/// Chi puo' ricevere un messaggio senza menzioni, oltre al titolare della
/// sessione attiva. Vuoto (`default`) se il thread non si legge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fallback {
    /// Il segretario autore del messaggio a cui l'utente risponde («Rispondi»).
    pub replied_bot: Option<String>,
    /// Il titolare persistito del thread (`Thread::owner`).
    pub thread_owner: Option<String>,
    /// L'ultimo segretario che ha scritto nel thread.
    pub last_bot: Option<String>,
}

impl Fallback {
    /// I candidati del thread per `message`: una risposta a un messaggio di
    /// sistema o dell'utente non ha `replied_bot`.
    pub fn of(thread: &Thread, message: &Message) -> Self {
        let replied_bot = message
            .reply_to
            .as_deref()
            .and_then(|id| thread.messages.iter().find(|m| m.id == id))
            .and_then(|m| bot_name(&m.author));
        let last_bot = thread.messages.iter().rev().find_map(|m| bot_name(&m.author));
        Self { replied_bot, thread_owner: thread.owner.clone(), last_bot }
    }

    /// I candidati in ordine di precedenza, con il titolare della sessione
    /// attiva (`session_owner`) subito dopo il segretario a cui si risponde.
    pub fn candidates<'a>(&'a self, session_owner: Option<&'a str>) -> impl Iterator<Item = &'a str> {
        [self.replied_bot.as_deref(), session_owner, self.thread_owner.as_deref(), self.last_bot.as_deref()]
            .into_iter()
            .flatten()
    }
}

fn bot_name(author: &Author) -> Option<String> {
    match author {
        Author::Bot { name } => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::{bot, proposal_event, reply_to, thread, user};

    #[test]
    fn a_reply_to_a_bot_names_its_author() {
        let mut t = thread(vec![bot("b1", "giorgio", "fatto"), bot("b2", "mario", "ok"), user("m1", "bene")]);
        t.owner = Some("anna".into());
        let fallback = Fallback::of(&t, &reply_to("m2", "b1", "e poi?"));
        assert_eq!(
            fallback,
            Fallback {
                replied_bot: Some("giorgio".into()),
                thread_owner: Some("anna".into()),
                last_bot: Some("mario".into()),
            }
        );
        assert_eq!(fallback.candidates(Some("luca")).collect::<Vec<_>>(), ["giorgio", "luca", "anna", "mario"]);
    }

    #[test]
    fn a_reply_to_an_event_or_a_missing_message_has_no_replied_bot() {
        let t = thread(vec![proposal_event("e1", "p1"), user("m1", "ciao")]);
        assert_eq!(Fallback::of(&t, &reply_to("m2", "e1", "accetta")), Fallback::default());
        assert_eq!(Fallback::of(&t, &reply_to("m2", "potato", "accetta")), Fallback::default());
        assert_eq!(Fallback::of(&t, &user("m2", "accetta")).candidates(None).count(), 0);
    }
}
