//! L'attesa dei supervisori, dietro un trait: in produzione `tokio::time::sleep`,
//! nei test un finto istantaneo che registra le attese. I timeout delle singole
//! chiamate invece usano il tempo vero di tokio (nei test non scattano mai).

use crate::connectors::contracts::{BoxFuture, ConnectorError};
use std::future::Future;
use std::time::Duration;

pub trait Sleeper: Send + Sync {
    /// Aspetta `duration`. `false` = il supervisore deve fermarsi (i test lo usano
    /// per chiudere un ciclo che altrimenti non finisce mai; in produzione e' sempre `true`).
    fn sleep(&self, duration: Duration) -> BoxFuture<'_, bool>;
}

pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, duration: Duration) -> BoxFuture<'_, bool> {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
            true
        })
    }
}

/// Esegue `future` entro `limit`; scaduto il tempo e' un errore transitorio.
pub async fn with_timeout<T>(
    limit: Duration,
    future: impl Future<Output = Result<T, ConnectorError>>,
) -> Result<T, ConnectorError> {
    match tokio::time::timeout(limit, future).await {
        Ok(result) => result,
        Err(_) => Err(ConnectorError::Transient("tempo scaduto".to_string())),
    }
}
