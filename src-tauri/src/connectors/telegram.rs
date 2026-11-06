//! Il connettore Telegram (`contact`): notifiche dell'HUB verso una chat privata con
//! un bot, o verso un supergruppo forum con un topic per thread, e messaggi
//! dell'utente ai segretari. E' un traduttore Bot API <-> DTO dei
//! contratti: collegamenti, dedupe, comandi eseguiti, thread, limiti degli allegati e
//! testi delle risposte ai comandi sono del runtime. Guida: docs/CONNECTORS.MD §13.
//!
//! Un file per responsabilita' (le parti pure hanno i test in `<modulo>/tests.rs`):
//! - `descriptor`: il descrittore statico e il filtro delle notifiche per tipo;
//! - `token`: formato e lettura del token del bot, URL della Bot API;
//! - `classify`: la classificazione pura di una risposta della Bot API in errore;
//! - `requests`: i corpi JSON delle chiamate;
//! - `api`: le chiamate alla Bot API sul client del contesto;
//! - `names`: nomi visualizzati e username ripuliti;
//! - `updates`: il parsing tollerante di `getUpdates`;
//! - `files`: allegati dei messaggi in entrata e risposta di `getFile`;
//! - `commands`: comandi e invito `/start <codice>`;
//! - `inbound`: aggiornamenti -> messaggi in entrata (album compresi);
//! - `cursor`: il cursore `{"offset": n}`;
//! - `receive`: un giro di long polling;
//! - `text`: il testo spezzato al limite di Telegram;
//! - `pacer`: la cadenza dei messaggi per chat;
//! - `ids`: gli id esterni `<chat>:<messaggio>`;
//! - `forum`: chat di gruppo, nomi e id dei topic;
//! - `send`: l'invio, `fetch`: lo scarico di un allegato;
//! - `subchats`: i topic come sotto-chat (apertura, nome, chiusura, eliminazione);
//! - `pairing`: la risposta a un tentativo di abbinamento;
//! - `group_pairing`: il controllo dei permessi del bot prima di un abbinamento da un gruppo;
//! - `actions`: le azioni `Prova` e `Abbina`.

mod actions;
mod api;
mod classify;
mod commands;
mod cursor;
mod descriptor;
mod fetch;
mod files;
mod forum;
mod group_pairing;
mod ids;
mod inbound;
mod names;
mod pacer;
mod pairing;
mod receive;
mod requests;
mod send;
mod subchats;
mod text;
mod token;
mod updates;

#[cfg(test)]
mod tests;

use crate::connectors::contracts::{
    ActionOutcome, BoxFuture, Connector, ConnectorContext, ConnectorError, ContactConnector, OutboundMessage,
    PairingOutcome, ReceiveBatch, RemoteFile,
};
use crate::connectors::descriptor::{ActionId, ConnectorDescriptor};
use crate::hub::attachments::IncomingFile;
use crate::hub::NoticeCategory;
use serde_json::Value;
use std::sync::Mutex;

pub struct Telegram {
    /// La cadenza dei messaggi per chat, condivisa fra le istanze del connettore.
    pacer: Mutex<pacer::Pacer>,
}

impl Telegram {
    const fn new() -> Self {
        Self { pacer: Mutex::new(pacer::Pacer::new()) }
    }
}

pub static TELEGRAM: Telegram = Telegram::new();

impl Connector for Telegram {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        &descriptor::DESCRIPTOR
    }

    fn migrate_config(&self, from: u32, _config: Value) -> Result<Value, String> {
        Err(format!("nessuna migrazione dalla versione {from}"))
    }

    fn validate_config(&self, _config: &Value) -> Result<(), String> {
        Ok(())
    }

    fn action<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        action: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(actions::run(ctx, action))
    }
}

impl ContactConnector for Telegram {
    fn wants_notice(&self, config: &Value, category: NoticeCategory) -> bool {
        descriptor::notice_wanted(config, category)
    }

    fn mirrors_app_messages(&self, config: &Value) -> bool {
        descriptor::mirror_wanted(config)
    }

    fn receive<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        cursor: Option<Value>,
    ) -> BoxFuture<'a, Result<ReceiveBatch, ConnectorError>> {
        Box::pin(receive::receive(&self.pacer, ctx, cursor))
    }

    fn send<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        out: &'a OutboundMessage,
    ) -> BoxFuture<'a, Result<Vec<String>, ConnectorError>> {
        Box::pin(send::send(&self.pacer, ctx, out))
    }

    fn fetch_file<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        file: &'a RemoteFile,
    ) -> BoxFuture<'a, Result<IncomingFile, ConnectorError>> {
        Box::pin(fetch::fetch_file(ctx, file))
    }

    fn pairing_reply<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        outcome: &'a PairingOutcome,
        chat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(pairing::reply(&self.pacer, ctx, outcome, chat))
    }

    fn subchats(&self, _config: &Value, chat: &str) -> bool {
        forum::is_supergroup(chat)
    }

    fn open_subchat<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        chat: &'a str,
        title: &'a str,
    ) -> BoxFuture<'a, Result<String, ConnectorError>> {
        Box::pin(subchats::open(&self.pacer, ctx, chat, title))
    }

    fn rename_subchat<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        chat: &'a str,
        subchat: &'a str,
        title: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(subchats::rename(&self.pacer, ctx, chat, subchat, title))
    }

    fn close_subchat<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        chat: &'a str,
        subchat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(subchats::close(&self.pacer, ctx, chat, subchat))
    }

    fn reopen_subchat<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        chat: &'a str,
        subchat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(subchats::reopen(&self.pacer, ctx, chat, subchat))
    }

    fn delete_subchat<'a>(
        &'a self,
        ctx: &'a ConnectorContext,
        chat: &'a str,
        subchat: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>> {
        Box::pin(subchats::delete(&self.pacer, ctx, chat, subchat))
    }
}
