//! La cadenza delle chiamate che scrivono in una chat (pura): Telegram tollera circa
//! un messaggio al secondo in una chat privata e circa 20 al minuto in un gruppo, dove
//! conta anche l'apertura di un topic. Dato l'istante, si prenota il prossimo posto
//! libero e si sa quanto aspettare. La mappa e' limitata: le chat che escono sono le
//! piu' vecchie.

use super::forum::is_group;
use std::collections::BTreeMap;

/// Distanza minima fra due messaggi alla stessa chat privata, in millisecondi.
pub const SLOT_GAP_MS: i64 = 1100;
/// Distanza minima fra due chiamate allo stesso gruppo (20 al minuto, con margine).
pub const GROUP_SLOT_GAP_MS: i64 = 3100;
/// Chat ricordate al massimo.
const CHATS_MAX: usize = 64;

/// La distanza fra due posti per la `chat`.
fn gap_for(chat: &str) -> i64 {
    if is_group(chat) {
        GROUP_SLOT_GAP_MS
    } else {
        SLOT_GAP_MS
    }
}

#[derive(Debug, Default)]
pub struct Pacer {
    /// Per chat: l'istante (ms) da cui il prossimo messaggio puo' partire.
    next_free: BTreeMap<String, i64>,
}

impl Pacer {
    pub const fn new() -> Self {
        Self { next_free: BTreeMap::new() }
    }

    /// Prenota il prossimo posto per `chat` all'istante `now` (ms) e ritorna
    /// quanti millisecondi aspettare prima di inviare (0 = subito).
    pub fn reserve(&mut self, chat: &str, now: i64) -> i64 {
        let slot = self.next_free.get(chat).copied().unwrap_or(now).max(now);
        self.next_free.insert(chat.to_string(), slot.saturating_add(gap_for(chat)));
        self.forget_oldest();
        slot.saturating_sub(now)
    }

    fn forget_oldest(&mut self) {
        while self.next_free.len() > CHATS_MAX {
            let oldest = self.next_free.iter().min_by_key(|(_, free)| **free).map(|(chat, _)| chat.clone());
            match oldest {
                Some(chat) => self.next_free.remove(&chat),
                None => break,
            };
        }
    }
}

#[cfg(test)]
mod tests;
