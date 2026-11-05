//! Il connettore Gmail (`source`): le email che corrispondono a una ricerca diventano
//! thread. E' un traduttore Gmail API <-> DTO dei contratti, in sola lettura
//! (`gmail.readonly`) e con OAuth 2.0 «desktop» (PKCE, redirect su loopback).
//! Guida: docs/CONNECTORS.MD §13.
//!
//! Un file per responsabilita' (le parti pure hanno i test in `<modulo>/tests.rs`):
//! - `descriptor`: il descrittore statico;
//! - `keys`: chiavi di configurazione, slot dei segreti, host e scope;
//! - `ascii`: il controllo dei valori ASCII visibili;
//! - `form`: la codifica `application/x-www-form-urlencoded`;
//! - `validate`: query, client id e configurazione;
//! - `pkce`: verifier, challenge e `state` del consenso;
//! - `consent`: l'URL della pagina di consenso e il redirect;
//! - `token`: i corpi e la risposta dell'endpoint dei token;
//! - `tokens`: la cache dei token di accesso;
//! - `access`: il client OAuth e il token di accesso (cache o rinnovo);
//! - `callback`: la richiesta di redirect su loopback e le sue risposte fisse;
//! - `loopback`: il server che aspetta il redirect del consenso;
//! - `browser`: l'apertura del browser;
//! - `authorize`: l'azione `Autorizza`;
//! - `actions`: le azioni `Autorizza` e `Prova`;
//! - `api_urls`: gli URL della Gmail API e la query di ricerca;
//! - `classify`: la risposta HTTP della Gmail API come errore del connettore;
//! - `api`: le chiamate GET alla Gmail API (token, nuovo tentativo su 401, tetti);
//! - `list`: la risposta di `messages.list`;
//! - `charset`, `words`: i set di caratteri e le parole codificate RFC 2047;
//! - `html`: dall'HTML al testo;
//! - `mime`: l'albero delle parti di un messaggio;
//! - `message`: da `messages.get` a un messaggio letto;
//! - `format`: titolo, mittente e corpo del thread;
//! - `cursor`: il cursore per regola;
//! - `plan`: quali messaggi scaricare in un giro;
//! - `profile`: la risposta di `getProfile` e gli esiti delle azioni;
//! - `listing`: gli id nuovi di ogni regola;
//! - `fetch`: lo scarico di un messaggio e dei suoi allegati;
//! - `attachments`: quali allegati si scaricano;
//! - `budget`: i budget di byte e di tempo di un giro;
//! - `items`: da un messaggio a un elemento della fonte;
//! - `gather`: la lettura dei messaggi pianificati;
//! - `poll`: un giro di `poll`.

mod access;
mod actions;
mod api;
mod api_urls;
mod ascii;
mod attachments;
mod authorize;
mod browser;
mod budget;
mod callback;
mod charset;
mod classify;
mod consent;
mod cursor;
mod descriptor;
mod fetch;
mod form;
mod format;
mod gather;
mod html;
mod items;
mod keys;
mod list;
mod listing;
mod loopback;
mod message;
mod mime;
mod pkce;
mod plan;
mod poll;
mod profile;
mod token;
mod tokens;
mod validate;
mod words;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

use crate::connectors::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, Connector, ConnectorContext, ConnectorError, PollBatch, SourceConnector,
};
use crate::connectors::descriptor::{ActionId, ConnectorDescriptor};
use serde_json::Value;
use std::sync::Mutex;

pub struct Gmail {
    /// I token di accesso, per istanza, condivisi fra i giri e le azioni.
    tokens: Mutex<tokens::TokenCache>,
}

impl Gmail {
    const fn new() -> Self {
        Self { tokens: Mutex::new(tokens::TokenCache::new()) }
    }
}

pub static GMAIL: Gmail = Gmail::new();

impl Connector for Gmail {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        &descriptor::DESCRIPTOR
    }

    fn migrate_config(&self, from: u32, _config: Value) -> Result<Value, String> {
        Err(format!("nessuna migrazione dalla versione {from}"))
    }

    fn validate_config(&self, config: &Value) -> Result<(), String> {
        validate::validate_config(config)
    }

    fn action<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        action: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(actions::run(&self.tokens, ctx, action))
    }
}

impl SourceConnector for Gmail {
    fn validate_filter(&self, filter: &Value) -> Result<(), String> {
        validate::validate_filter(filter)
    }

    fn poll<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        routes: &'a [ActiveRoute],
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<PollBatch, ConnectorError>> {
        Box::pin(poll::poll(&self.tokens, ctx, routes, cursor))
    }
}
