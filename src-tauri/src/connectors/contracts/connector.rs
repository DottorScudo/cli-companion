//! I trait di un connettore. Il connettore e' un traduttore servizio <-> DTO:
//! non consegna nell'HUB (lo fa il supervisore, cosi' dedupe, limiti, risvegli e
//! permessi stanno in un posto solo) e non costruisce un client di rete (usa
//! `ctx.http()`). Vedi docs/CONNECTORS.MD §4.

use super::{
    ActionOutcome, ActiveRoute, BoxFuture, ConnectorContext, ConnectorError, OutboundMessage, PairingOutcome,
    PollBatch, ReceiveBatch, RemoteFile,
};
use crate::connectors::descriptor::{ActionId, ConnectorDescriptor};
use crate::hub::attachments::IncomingFile;
use crate::hub::NoticeCategory;
use serde_json::Value;

pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &'static ConnectorDescriptor;

    /// Pura: la configurazione salvata con la versione `from` diventa quella della
    /// versione `from + 1`, di UN passo; il runtime la chiama in sequenza fino alla
    /// `version` corrente del descrittore. Vale anche per i filtri delle regole.
    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String>;

    /// Validazione semantica (la generica, di tipi e limiti, e' gia' stata fatta da
    /// `descriptor::validate`).
    fn validate_config(&self, config: &Value) -> Result<(), String>;

    /// Un'azione dell'interfaccia. Arrivano solo quelle dichiarate nel descrittore.
    fn action<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        action: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>>;
}

/// Un connettore `source`: elementi esterni verso i thread.
pub trait SourceConnector: Connector {
    fn validate_filter(&self, filter: &Value) -> Result<(), String>;

    /// Un giro: legge gli elementi nuovi per le regole attive a partire da `cursor`
    /// (`None` = si parte da "adesso"). Non consegna: restituisce gli elementi e il
    /// cursore nuovo, che il supervisore salva dopo la consegna.
    fn poll<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        routes: &'a [ActiveRoute],
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<PollBatch, ConnectorError>>;
}

/// Un connettore `contact`: notifiche verso l'utente e suoi messaggi ai segretari.
pub trait ContactConnector: Connector {
    /// Pura: la configurazione (gia' validata) chiede di inoltrare le notifiche di questo tipo?
    fn wants_notice(&self, config: &Value, category: NoticeCategory) -> bool;

    /// Long polling: aspetta (con un timeout, al piu' `limits::RECEIVE_TIMEOUT`)
    /// i messaggi in entrata e restituisce anche il cursore nuovo, che il
    /// supervisore salva dopo la consegna. Non scarica gli allegati (`fetch_file`).
    fn receive<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<ReceiveBatch, ConnectorError>>;

    /// Manda un messaggio a una chat collegata, spezzandolo se supera il limite del
    /// servizio e rispettandone la frequenza; ritorna l'id esterno di ogni parte
    /// inviata, in ordine (nessuno solo per un testo vuoto).
    fn send<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        out: &'a OutboundMessage,
    ) -> BoxFuture<'a, Result<Vec<String>, ConnectorError>>;

    /// Scarica un allegato di un messaggio in entrata. Il runtime lo chiede solo
    /// per un account collegato ed entro i limiti degli allegati dell'HUB.
    fn fetch_file<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        file: &'a RemoteFile,
    ) -> BoxFuture<'a, Result<IncomingFile, ConnectorError>>;

    /// Un account ha presentato un codice di abbinamento dalla `chat`: il
    /// connettore risponde come crede (benvenuto, errore generico).
    fn pairing_reply<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        outcome: &'a PairingOutcome,
        chat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>>;
}

/// Un connettore registrato, col suo tipo.
#[derive(Clone, Copy)]
pub enum ConnectorImpl {
    Source(&'static dyn SourceConnector),
    Contact(&'static dyn ContactConnector),
}

impl ConnectorImpl {
    pub fn connector(&self) -> &'static dyn Connector {
        match *self {
            Self::Source(source) => source,
            Self::Contact(contact) => contact,
        }
    }

    pub fn descriptor(&self) -> &'static ConnectorDescriptor {
        self.connector().descriptor()
    }

    /// Il tipo secondo la variante: deve coincidere con `descriptor().kind` (lo verifica la conformita').
    #[cfg(test)]
    pub fn kind(&self) -> crate::connectors::descriptor::ConnectorKind {
        match self {
            Self::Source(_) => crate::connectors::descriptor::ConnectorKind::Source,
            Self::Contact(_) => crate::connectors::descriptor::ConnectorKind::Contact,
        }
    }
}
