//! Chiusura dei terminali all'uscita dell'app.

use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::registry::{PtySession, SharedRegistry};

/// Attesa massima, in tutto, della chiusura dei terminali all'uscita: la
/// chiusura di una pseudo-console puo' bloccarsi e non deve tenere l'app aperta.
pub(super) const EXIT_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

/// Impedisce nuove sessioni e termina quelle vive, ognuna sul proprio thread,
/// attendendo al massimo `timeout`.
pub(super) fn close_all(registry: &SharedRegistry, timeout: Duration) {
    let sessions = registry.lock().unwrap().begin_close();
    run_within(sessions, timeout, PtySession::shutdown);
}

/// Esegue `work` su ogni elemento, uno per thread, e attende al massimo
/// `timeout` in totale. Ritorna quanti hanno finito in tempo: i thread in
/// ritardo non vengono attesi.
pub(super) fn run_within<T, F>(items: Vec<T>, timeout: Duration, work: F) -> usize
where
    T: Send + 'static,
    F: Fn(T) + Send + Sync + 'static,
{
    let total = items.len();
    let work = Arc::new(work);
    let (done_tx, done_rx) = channel();
    for item in items {
        let (work, done_tx) = (work.clone(), done_tx.clone());
        std::thread::spawn(move || {
            work(item);
            let _ = done_tx.send(());
        });
    }
    drop(done_tx);
    let deadline = Instant::now() + timeout;
    let mut finished = 0;
    while finished < total && done_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).is_ok() {
        finished += 1;
    }
    finished
}
