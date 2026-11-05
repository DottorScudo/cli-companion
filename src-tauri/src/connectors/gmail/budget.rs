//! I budget di un giro (puri): byte di allegati e tempo. Esauriti, il giro si ferma e il
//! prossimo riparte subito. Si elabora sempre almeno un messaggio, cosi' un giro avanza comunque.

use crate::connectors::limits::POLL_TIMEOUT;

/// Byte di allegati scaricati in un giro, al massimo: un messaggio al limite dell'HUB
/// (5 allegati da 10 MB) ci sta da solo, e la memoria resta limitata.
pub const ROUND_BYTES_MAX: u64 = 50 * 1024 * 1024;
/// Tempo di un giro dopo il quale non si inizia un altro messaggio, in millisecondi. Sotto
/// `POLL_TIMEOUT` (90 s) per lasciare il tempo di finire il messaggio in corso.
pub const ROUND_TIME_MAX_MS: i64 = 60_000;

const _: () = assert!((ROUND_TIME_MAX_MS as u128) < POLL_TIMEOUT.as_millis());

#[derive(Debug)]
pub struct Budget {
    started_ms: i64,
    bytes_used: u64,
    messages_done: usize,
}

impl Budget {
    pub const fn new(started_ms: i64) -> Self {
        Self { started_ms, bytes_used: 0, messages_done: 0 }
    }

    /// Il tempo e' finito? Mai prima del primo messaggio elaborato.
    pub fn time_up(&self, now_ms: i64) -> bool {
        self.messages_done > 0 && now_ms.saturating_sub(self.started_ms) >= ROUND_TIME_MAX_MS
    }

    /// Ci stanno ancora `bytes` di allegati? Il primo scarico del giro passa sempre.
    pub fn admits(&self, bytes: u64) -> bool {
        self.bytes_used == 0 || self.bytes_used.saturating_add(bytes) <= ROUND_BYTES_MAX
    }

    pub fn spend(&mut self, bytes: u64) {
        self.bytes_used = self.bytes_used.saturating_add(bytes);
    }

    /// Un messaggio e' stato elaborato (consegnato o scartato).
    pub fn message_done(&mut self) {
        self.messages_done = self.messages_done.saturating_add(1);
    }
}

#[cfg(test)]
mod tests;
