//! Il server in esecuzione: una riga di stdin alla volta, risposte e notifiche
//! su uno scrittore condiviso col thread dei tentativi di `whoami`. Ogni
//! messaggio e' una riga JSON intera, scritta sotto lock: mai intercalate.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::retry::{retry_role, Clock, RetryPolicy};
use super::state::Server;
use super::{handle_line, Backend};
use crate::automation::lock;

/// Avviso al client: la lista dei tool e' cambiata, va chiesta di nuovo.
fn list_changed() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
}

/// Scrive `msg` come una riga JSON e svuota il buffer.
fn write_message(out: &mut impl Write, msg: &Value) -> io::Result<()> {
    let mut line = serde_json::to_string(msg).map_err(io::Error::other)?;
    line.push('\n');
    out.write_all(line.as_bytes())?;
    out.flush()
}

/// Ciclo di vita del server sopra il protocollo di `handle_line`.
pub struct Session<W> {
    server: Server,
    backend: Arc<dyn Backend>,
    out: Arc<Mutex<W>>,
    clock: Arc<dyn Clock>,
    policy: RetryPolicy,
}

impl<W: Write + Send + 'static> Session<W> {
    pub fn new(backend: Arc<dyn Backend>, out: Arc<Mutex<W>>, clock: Arc<dyn Clock>, policy: RetryPolicy) -> Self {
        Self { server: Server::default(), backend, out, clock, policy }
    }

    /// Gestisce una riga di stdin e scrive la risposta. Se ora servono i
    /// tentativi di `whoami` ritorna il lavoro da far girare in background.
    /// Un errore di scrittura vuol dire client sparito.
    pub fn respond(&self, line: &str) -> io::Result<Option<RetryJob<W>>> {
        let reply = handle_line(&self.server, line, self.backend.as_ref());
        self.deliver(reply.as_ref())?;
        Ok(self.server.claim_retry().then(|| RetryJob {
            server: self.server.clone(),
            backend: Arc::clone(&self.backend),
            out: Arc::clone(&self.out),
            clock: Arc::clone(&self.clock),
            policy: self.policy,
        }))
    }

    /// Scrive la risposta e, subito dopo e sotto lo stesso lock, l'eventuale
    /// `list_changed`: cosi' non precede mai la lista vuota che la motiva.
    fn deliver(&self, reply: Option<&Value>) -> io::Result<()> {
        let mut out = lock(&self.out);
        if let Some(reply) = reply {
            write_message(&mut *out, reply)?;
        }
        if self.server.reply_written() {
            write_message(&mut *out, &list_changed())?;
        }
        Ok(())
    }
}

/// Tentativi di `whoami` in background, con quel che serve per avvisare il client.
pub struct RetryJob<W> {
    server: Server,
    backend: Arc<dyn Backend>,
    out: Arc<Mutex<W>>,
    clock: Arc<dyn Clock>,
    policy: RetryPolicy,
}

impl<W: Write> RetryJob<W> {
    /// Riprova finche' il ruolo arriva o il budget finisce, poi avvisa il
    /// client se ha in mano una lista vuota.
    pub fn run(self) {
        if !retry_role(&self.server, self.backend.as_ref(), self.clock.as_ref(), &self.policy) {
            return;
        }
        if self.server.take_list_changed() {
            // Una scrittura fallita vuol dire client sparito: il ciclo
            // principale lo vede alla prossima risposta e chiude.
            let _ = write_message(&mut *lock(&self.out), &list_changed());
        }
    }
}

#[cfg(test)]
mod tests;
