//! I contratti dei connettori: i trait che un connettore implementa, gli errori
//! con cui si fa classificare dal supervisore, i DTO che scambia con il runtime e
//! il contesto che il runtime gli da'. Stessa forma dei contratti degli agenti
//! (`agents/contracts/`): oggetti `&'static dyn`, futuri boxati senza il crate
//! `futures`, nessuna implementazione di default che nasconda una lacuna.

mod connector;
mod context;
mod error;
mod items;
#[cfg(test)]
mod tests;

pub use connector::{Connector, ConnectorImpl, ContactConnector, SourceConnector};
pub use context::{Clock, ConnectorContext, ContextParts, HubPort, StateHandle, SystemClock};
pub use error::ConnectorError;
pub use items::{
    ActionOutcome, ActiveRoute, InboundMessage, OutboundMessage, PairingOutcome, PollBatch, ReceiveBatch, SourceItem,
};

use std::future::Future;
use std::pin::Pin;

/// Un futuro boxato, `Send`: i trait dei connettori sono usati come `dyn`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
