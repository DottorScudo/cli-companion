//! I contratti dei connettori: i trait che un connettore implementa, gli errori
//! con cui si fa classificare dal supervisore, i DTO che scambia con il runtime e
//! il contesto che il runtime gli da'. Stessa forma dei contratti degli agenti
//! (`agents/contracts/`): oggetti `&'static dyn`, futuri boxati senza il crate
//! `futures`, nessuna implementazione di default che nasconda una lacuna. Fanno
//! eccezione le sotto-chat dei canali di contatto, una capacita' facoltativa: i metodi
//! predefiniti dicono «non supportate» e il runtime non le chiede.

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
    ActionOutcome, ActiveRoute, ContactCommand, InboundMessage, OutboundMessage, PairingOutcome, PollBatch,
    ReceiveBatch, RemoteFile, SourceItem,
};

use std::future::Future;
use std::pin::Pin;

/// Un futuro boxato, `Send`: i trait dei connettori sono usati come `dyn`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
