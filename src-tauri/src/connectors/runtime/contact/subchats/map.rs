//! La mappa fra i thread dell'HUB e le sotto-chat delle chat collegate, nei due
//! versi: `(thread, chat) -> sotto-chat` per le notifiche, `(chat, sotto-chat) ->
//! thread` per i messaggi in entrata. Lo stesso thread puo' avere una sotto-chat in
//! ogni chat collegata. Pura e limitata: al piu' `cap` voci, la meno recente (per
//! inserimento o per uso) esce per prima, sempre dai due versi insieme. Si salva
//! nello stato dell'istanza come elenco di voci, dalla meno recente alla piu'
//! recente; una voce non valida letta dal file si scarta.

use crate::connectors::limits::{SUBCHATS_MAX, SUBCHAT_ID_MAX};
use crate::connectors::model::valid_external_ref;
use crate::hub::{valid_ref_id, Channel};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Caratteri ammessi in un id di sotto-chat oltre a lettere e cifre ASCII.
const SUBCHAT_ID_PUNCTUATION: [char; 4] = ['-', '_', '.', ':'];

/// L'id di una sotto-chat che arriva dal servizio si puo' salvare? Da 1 a
/// `SUBCHAT_ID_MAX` caratteri ASCII fra lettere, cifre e `-_.:`.
pub fn valid_subchat_id(id: &str) -> bool {
    (1..=SUBCHAT_ID_MAX).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || SUBCHAT_ID_PUNCTUATION.contains(&c))
}

/// Un thread dell'HUB.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadKey {
    pub channel: Channel,
    pub thread_id: String,
}

/// Una sotto-chat di una chat collegata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubchatKey {
    pub chat: String,
    pub subchat: String,
}

/// Una voce come si salva su disco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubchatEntry {
    pub channel: Channel,
    pub thread_id: String,
    pub chat: String,
    pub subchat: String,
    /// Chiusa nel servizio (il thread e' concluso). Assente nel file = aperta.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
}

/// La chiave del verso thread -> sotto-chat: il thread e la chat.
type ThreadInChat = (ThreadKey, String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<SubchatEntry>", into = "Vec<SubchatEntry>")]
pub struct SubchatMap {
    cap: usize,
    /// Le sotto-chat mappate, dalla meno recente alla piu' recente.
    order: VecDeque<SubchatKey>,
    threads: HashMap<SubchatKey, ThreadKey>,
    subchats: HashMap<ThreadInChat, String>,
    /// Le sotto-chat mappate che il runtime ha chiuso.
    closed: HashSet<SubchatKey>,
}

impl SubchatMap {
    pub fn with_capacity(cap: usize) -> Self {
        Self { cap, order: VecDeque::new(), threads: HashMap::new(), subchats: HashMap::new(), closed: HashSet::new() }
    }

    /// Le sotto-chat dei thread per cui `selected` e' vero, senza cambiarne l'ordine d'uso.
    pub fn places_where(&self, selected: impl Fn(&ThreadKey) -> bool) -> Vec<SubchatKey> {
        self.order.iter().filter(|place| self.threads.get(*place).is_some_and(&selected)).cloned().collect()
    }

    /// La sotto-chat e' chiusa?
    pub fn is_closed(&self, place: &SubchatKey) -> bool {
        self.closed.contains(place)
    }

    /// Segna la sotto-chat chiusa o riaperta. Non fa nulla se non e' mappata.
    pub fn set_closed(&mut self, place: &SubchatKey, closed: bool) {
        if !self.threads.contains_key(place) {
            return;
        }
        if closed {
            self.closed.insert(place.clone());
        } else {
            self.closed.remove(place);
        }
    }

    /// La sotto-chat del thread nella `chat`, se c'e': la voce diventa la piu' recente.
    pub fn subchat_of(&mut self, thread: &ThreadKey, chat: &str) -> Option<String> {
        let subchat = self.subchats.get(&(thread.clone(), chat.to_string()))?.clone();
        self.move_to_newest(&SubchatKey { chat: chat.to_string(), subchat: subchat.clone() });
        Some(subchat)
    }

    /// Il thread di una sotto-chat, se c'e': la voce diventa la piu' recente.
    pub fn thread_of(&mut self, place: &SubchatKey) -> Option<ThreadKey> {
        let thread = self.threads.get(place)?.clone();
        self.move_to_newest(place);
        Some(thread)
    }

    /// Lega il thread alla sotto-chat, togliendo le voci che li legavano ad altro.
    /// `false` (e nulla cambia) se un id non e' valido.
    pub fn insert(&mut self, thread: ThreadKey, place: SubchatKey) -> bool {
        if !valid_ref_id(&thread.thread_id) || !valid_external_ref(&place.chat) || !valid_subchat_id(&place.subchat) {
            return false;
        }
        self.remove_subchat(&place);
        if let Some(previous) = self.subchats.get(&(thread.clone(), place.chat.clone())).cloned() {
            self.remove_subchat(&SubchatKey { chat: place.chat.clone(), subchat: previous });
        }
        self.subchats.insert((thread.clone(), place.chat.clone()), place.subchat.clone());
        self.threads.insert(place.clone(), thread);
        self.order.push_back(place);
        while self.order.len() > self.cap {
            let Some(oldest) = self.order.pop_front() else { break };
            self.closed.remove(&oldest);
            if let Some(thread) = self.threads.remove(&oldest) {
                self.subchats.remove(&(thread, oldest.chat));
            }
        }
        true
    }

    /// Toglie la sotto-chat dai due versi. Ritorna il thread a cui era legata.
    pub fn remove_subchat(&mut self, place: &SubchatKey) -> Option<ThreadKey> {
        let thread = self.threads.remove(place)?;
        self.subchats.remove(&(thread.clone(), place.chat.clone()));
        self.closed.remove(place);
        self.order.retain(|k| k != place);
        Some(thread)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    fn move_to_newest(&mut self, place: &SubchatKey) {
        self.order.retain(|k| k != place);
        self.order.push_back(place.clone());
    }
}

impl Default for SubchatMap {
    fn default() -> Self {
        Self::with_capacity(SUBCHATS_MAX)
    }
}

impl From<Vec<SubchatEntry>> for SubchatMap {
    /// Da un file: le voci non valide si scartano, oltre il tetto restano le piu' recenti.
    fn from(list: Vec<SubchatEntry>) -> Self {
        let mut map = Self::default();
        for entry in list {
            let thread = ThreadKey { channel: entry.channel, thread_id: entry.thread_id };
            let place = SubchatKey { chat: entry.chat, subchat: entry.subchat };
            if map.insert(thread, place.clone()) {
                map.set_closed(&place, entry.closed);
            }
        }
        map
    }
}

impl From<SubchatMap> for Vec<SubchatEntry> {
    fn from(map: SubchatMap) -> Self {
        let SubchatMap { order, mut threads, closed, .. } = map;
        order
            .into_iter()
            .filter_map(|place| {
                let thread = threads.remove(&place)?;
                Some(SubchatEntry {
                    channel: thread.channel,
                    thread_id: thread.thread_id,
                    closed: closed.contains(&place),
                    chat: place.chat,
                    subchat: place.subchat,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
