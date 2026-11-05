//! Il server sul loopback che aspetta il redirect del consenso. Una connessione alla volta,
//! generica su `AsyncRead + AsyncWrite` (si prova con `tokio::io::duplex`). Il codice e lo
//! `state` non finiscono mai in un errore o in un log.

use super::callback::{head_end, parse_request, response, Callback, Page, HEAD_MAX};
use super::pkce::states_match;
use crate::connectors::contracts::ConnectorError;
use crate::connectors::limits::ACTION_TIMEOUT;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

/// Quanto si aspetta il consenso dell'utente. Sotto `ACTION_TIMEOUT` (300 s) per lasciare il
/// tempo allo scambio del codice e al profilo (una chiamata ciascuno, al piu' 30 s).
pub const AUTH_WAIT: Duration = Duration::from_secs(240);
/// Tempo concesso a una connessione per mandare la richiesta e ricevere la risposta.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
/// Byte letti per volta dal socket.
const READ_CHUNK: usize = 1024;

const _: () = assert!(AUTH_WAIT.as_secs() + 2 * 30 <= ACTION_TIMEOUT.as_secs());

/// Cosa decidere dopo una connessione.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Non era il redirect (favicon, richiesta malformata, connessione chiusa): si continua ad aspettare.
    KeepWaiting,
    /// Il redirect del consenso, con lo `state` giusto.
    Code(String),
    /// L'utente ha negato il consenso (con lo `state` giusto).
    Denied,
    /// Un redirect con lo `state` sbagliato o assente: l'autorizzazione fallisce.
    Invalid,
}

/// La testata della richiesta, al piu' `HEAD_MAX` byte; `None` se la connessione si chiude prima.
async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; READ_CHUNK];
    loop {
        if let Some(end) = head_end(&buf) {
            buf.truncate(end);
            return Some(buf);
        }
        if buf.len() >= HEAD_MAX {
            return None;
        }
        let read = stream.read(&mut chunk).await.ok().filter(|n| *n > 0)?;
        buf.extend_from_slice(chunk.get(..read)?);
    }
}

fn good_state(expected: &str, got: Option<&str>) -> bool {
    got.is_some_and(|state| states_match(expected, state))
}

/// La pagina da mostrare e cosa fare, secondo la richiesta.
fn decide(request: Option<Callback>, expected_state: &str) -> (Page, Verdict) {
    match request {
        Some(Callback::Code { code, state }) if good_state(expected_state, Some(&state)) => {
            (Page::Success, Verdict::Code(code))
        }
        Some(Callback::Denied { state }) if good_state(expected_state, state.as_deref()) => {
            (Page::Failure, Verdict::Denied)
        }
        Some(Callback::Code { .. } | Callback::Denied { .. }) => (Page::Failure, Verdict::Invalid),
        Some(Callback::Other) | None => (Page::NotFound, Verdict::KeepWaiting),
    }
}

async fn reply<S: AsyncWrite + Unpin>(stream: &mut S, page: Page) {
    let _ = stream.write_all(&response(page)).await;
    let _ = stream.shutdown().await;
}

/// Serve una connessione: legge la richiesta, risponde con una pagina fissa e dice cosa fare.
pub async fn handle<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S, expected_state: &str) -> Verdict {
    let serve = async {
        let head = read_head(stream).await;
        let request = head.and_then(|h| parse_request(&h).ok());
        let (page, verdict) = decide(request, expected_state);
        reply(stream, page).await;
        verdict
    };
    timeout(CONNECTION_TIMEOUT, serve).await.unwrap_or(Verdict::KeepWaiting)
}

fn failure(message: &str) -> ConnectorError {
    ConnectorError::Fatal(message.to_string())
}

/// Aspetta il redirect fino a `AUTH_WAIT`: ritorna il codice di autorizzazione.
pub async fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<String, ConnectorError> {
    let accepting = async {
        loop {
            let (mut stream, _) =
                listener.accept().await.map_err(|_| failure("ascolto dell'autorizzazione non riuscito"))?;
            match handle(&mut stream, expected_state).await {
                Verdict::KeepWaiting => {}
                Verdict::Code(code) => return Ok(code),
                Verdict::Denied => return Err(failure("consenso negato")),
                Verdict::Invalid => return Err(failure("risposta di autorizzazione non valida")),
            }
        }
    };
    timeout(AUTH_WAIT, accepting).await.unwrap_or_else(|_| Err(failure("tempo scaduto per l'autorizzazione")))
}

#[cfg(test)]
mod tests;
