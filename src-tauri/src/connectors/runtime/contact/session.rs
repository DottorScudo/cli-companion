//! La memoria di conversazione in RAM di ogni account collegato: l'ultimo
//! elenco di `/threads` (per `/use <n>`), l'ultimo di `/ws` e il limite dei
//! messaggi in entrata. Si perde al riavvio del supervisore, volutamente: sono
//! comodita' di una conversazione, non dati. La logica per account
//! (`AccountSession`) e' pura; `ContactSession` la tiene dietro un lock mai
//! preso durante un'attesa.

use crate::hub::contact::ChannelEntry;
use crate::hub::Channel;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

/// Messaggi in entrata accettati da un account in un minuto (finestra mobile).
pub const INBOUND_PER_MINUTE: usize = 20;
const WINDOW_MS: i64 = 60_000;

/// Una riga dell'ultimo elenco di `/threads`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedThread {
    pub channel: Channel,
    pub thread_id: String,
    /// Come l'utente l'ha vista nell'elenco (`canale › titolo`).
    pub label: String,
}

/// L'esito del controllo del limite per un messaggio in entrata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Allowed,
    /// Oltre il limite: scartato. `first` e' vero solo per il primo scarto della finestra.
    Dropped {
        first: bool,
    },
}

#[derive(Debug, Default)]
pub struct AccountSession {
    threads: Vec<ListedThread>,
    channels: Vec<ChannelEntry>,
    /// Istanti dei messaggi accettati nell'ultimo minuto (al massimo `INBOUND_PER_MINUTE`).
    recent: VecDeque<i64>,
    /// Il primo scarto di questa finestra e' gia' stato annunciato.
    warned: bool,
}

impl AccountSession {
    pub fn remember_threads(&mut self, threads: Vec<ListedThread>) {
        self.threads = threads;
    }

    /// La riga `n` (da 1) dell'ultimo elenco di thread.
    pub fn thread_at(&self, n: usize) -> Option<&ListedThread> {
        self.threads.get(n.checked_sub(1)?)
    }

    pub fn listed_threads(&self) -> usize {
        self.threads.len()
    }

    pub fn remember_channels(&mut self, channels: Vec<ChannelEntry>) {
        self.channels = channels;
    }

    pub fn listed_channels(&self) -> &[ChannelEntry] {
        &self.channels
    }

    /// Un messaggio in entrata all'istante `now`: accettato o scartato per il limite.
    pub fn admit(&mut self, now: i64) -> Admission {
        while self.recent.front().is_some_and(|at| now.saturating_sub(*at) >= WINDOW_MS) {
            self.recent.pop_front();
        }
        if self.recent.len() < INBOUND_PER_MINUTE {
            self.recent.push_back(now);
            self.warned = false;
            return Admission::Allowed;
        }
        let first = !self.warned;
        self.warned = true;
        Admission::Dropped { first }
    }
}

/// Le sessioni di tutti gli account di un'istanza.
#[derive(Default)]
pub struct ContactSession {
    accounts: Mutex<HashMap<String, AccountSession>>,
}

impl ContactSession {
    /// Esegue `f` sulla sessione di `account` (creata alla prima volta). Il lock si tiene solo per `f`.
    pub fn with<R>(&self, account: &str, f: impl FnOnce(&mut AccountSession) -> R) -> R {
        let mut accounts = self.accounts.lock().unwrap_or_else(PoisonError::into_inner);
        f(accounts.entry(account.to_string()).or_default())
    }

    /// L'account non e' piu' collegato: la sua memoria si butta.
    pub fn forget(&self, account: &str) {
        self.accounts.lock().unwrap_or_else(PoisonError::into_inner).remove(account);
    }
}

#[cfg(test)]
mod tests;
