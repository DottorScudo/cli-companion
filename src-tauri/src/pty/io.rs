//! Flussi di una sessione: thread di scrittura (coda ordinata verso la
//! shell) e thread di lettura (output decodificato verso il frontend).

use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use tauri::ipc::Channel;

use super::PtyEvent;

const READ_BUFFER_BYTES: usize = 8192;

/// Avvia il thread che svuota la coda verso `writer`, nell'ordine di arrivo.
/// Termina quando il canale si chiude (sessione rimossa) o una scrittura fallisce.
pub(super) fn spawn_writer(mut writer: Box<dyn Write + Send>) -> Sender<Vec<u8>> {
    let (tx, rx) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        while let Ok(data) = rx.recv() {
            if writer.write_all(&data).and_then(|_| writer.flush()).is_err() {
                break;
            }
        }
    });
    tx
}

/// Avvia il thread che inoltra l'output di `reader` al canale della sessione.
/// A fine flusso esegue `on_closed` (rimozione della sessione, attesa del
/// figlio) e poi notifica `PtyEvent::Exit`.
pub(super) fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    on_event: Channel<PtyEvent>,
    on_closed: impl FnOnce() + Send + 'static,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; READ_BUFFER_BYTES];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let text = decode_incremental(&mut pending);
                    if !text.is_empty() {
                        let _ = on_event.send(PtyEvent::Output { data: text });
                    }
                }
            }
        }
        if !pending.is_empty() {
            let text = String::from_utf8_lossy(&pending).into_owned();
            let _ = on_event.send(PtyEvent::Output { data: text });
        }
        on_closed();
        let _ = on_event.send(PtyEvent::Exit);
    });
}

/// Decodifica UTF-8 incrementale: i chunk del pty possono spezzare un carattere
/// multibyte (accenti nei path, box drawing). I byte incompleti restano in coda.
pub(super) fn decode_incremental(pending: &mut Vec<u8>) -> String {
    let mut out = String::new();
    loop {
        if pending.is_empty() {
            break;
        }
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                out.push_str(valid);
                pending.clear();
                break;
            }
            Err(e) => {
                let up_to = e.valid_up_to();
                if up_to > 0 {
                    out.push_str(&String::from_utf8_lossy(&pending[..up_to]));
                    pending.drain(..up_to);
                } else if e.error_len().is_none() {
                    break; // sequenza incompleta: aspetto altri byte
                } else {
                    out.push('\u{FFFD}');
                    pending.drain(..1);
                }
            }
        }
    }
    out
}
