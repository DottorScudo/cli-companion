//! Esecuzione di processi con timeout e output limitato.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// Byte tenuti per ciascuno stream (stdout, stderr): il resto si legge e si
/// scarta, cosi' un figlio loquace non si blocca su una pipe piena ne' fa
/// crescere la memoria senza limite.
pub(crate) const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
/// Intervallo di controllo dell'uscita del figlio.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Attesa dello svuotamento delle pipe dopo l'uscita (o il kill) del figlio.
const DRAIN_GRACE: Duration = Duration::from_secs(1);
const READ_CHUNK_BYTES: usize = 8192;

/// Stream di un figlio letto da un thread dedicato.
struct Drain {
    buf: Arc<Mutex<Vec<u8>>>,
    done: mpsc::Receiver<()>,
}

impl Drain {
    fn spawn<R: Read + Send + 'static>(pipe: Option<R>, limit: usize) -> Self {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let (tx, done) = mpsc::channel();
        if let Some(mut pipe) = pipe {
            let shared = buf.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; READ_CHUNK_BYTES];
                while let Ok(n) = pipe.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let mut kept = shared.lock().unwrap();
                    let room = limit.saturating_sub(kept.len());
                    kept.extend_from_slice(&chunk[..n.min(room)]);
                }
                let _ = tx.send(());
            });
        } else {
            let _ = tx.send(());
        }
        Self { buf, done }
    }

    /// Attende la fine della lettura (al massimo `DRAIN_GRACE`: un nipote che
    /// tiene aperta la pipe non deve bloccare) e ritorna quanto raccolto.
    fn collect(&self) -> Vec<u8> {
        let _ = self.done.recv_timeout(DRAIN_GRACE);
        std::mem::take(&mut *self.buf.lock().unwrap())
    }
}

/// Come `Command::output` ma con timeout: allo scadere il figlio viene
/// ucciso e si ritorna `TimedOut`. stdin e' nullo; di ogni stream si tengono
/// al piu' `MAX_OUTPUT_BYTES`.
pub(crate) fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> std::io::Result<Output> {
    output_with_limit(cmd, timeout, MAX_OUTPUT_BYTES)
}

fn output_with_limit(cmd: &mut Command, timeout: Duration, limit: usize) -> std::io::Result<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let stdout = Drain::spawn(child.stdout.take(), limit);
    let stderr = Drain::spawn(child.stderr.take(), limit);

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    let (stdout, stderr) = (stdout.collect(), stderr.collect());
    match status {
        Some(status) => Ok(Output { status, stdout, stderr }),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("nessuna risposta entro {}s", timeout.as_secs()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn output_with_timeout_kills_hung_child_and_captures_output() {
        let mut slow = Command::new("ping");
        slow.args(["-n", "30", "127.0.0.1"]);
        let t = Instant::now();
        let err = output_with_timeout(&mut slow, Duration::from_millis(300)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(t.elapsed() < Duration::from_secs(5));

        let mut ok = Command::new("cmd");
        ok.args(["/c", "echo ciao"]);
        let out = output_with_timeout(&mut ok, Duration::from_secs(10)).unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("ciao"));
    }

    #[cfg(windows)]
    #[test]
    fn output_is_truncated_to_the_limit_without_blocking_the_child() {
        let mut cmd = Command::new("cmd");
        cmd.args(["/c", "echo 0123456789 & echo 0123456789 & echo 0123456789"]);
        let out = output_with_limit(&mut cmd, Duration::from_secs(10), 16).unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 16);
    }

    #[test]
    fn drain_keeps_at_most_the_limit_and_reads_the_whole_stream() {
        let source = std::io::repeat(b'x').take(100_000);
        let drain = Drain::spawn(Some(source), 1000);
        assert_eq!(drain.collect().len(), 1000);
    }

    #[test]
    fn drain_without_a_pipe_is_empty() {
        let drain = Drain::spawn(None::<std::io::Empty>, 10);
        assert!(drain.collect().is_empty());
    }
}
