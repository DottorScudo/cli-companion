//! La lettura dei messaggi pianificati in un giro, uno alla volta, con i budget di byte e di
//! tempo. Un messaggio che piu' regole corrispondono si legge (e si scarica) una volta sola;
//! ogni regola ne ricava il proprio elemento.

use super::attachments::{select, wanted_bytes, Files};
use super::budget::Budget;
use super::cursor::Cursor;
use super::fetch::{self, Fetched};
use super::items::deliver;
use super::message::ParsedMessage;
use super::tokens::TokenCache;
use crate::connectors::contracts::{ConnectorContext, ConnectorError, SourceItem};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// L'esito di un passo.
#[derive(Debug)]
pub enum Step {
    Item(Box<SourceItem>),
    /// Sparito, illeggibile o precedente alla regola: ricordato, nessun elemento.
    Skipped,
    /// Un budget e' esaurito: il giro si ferma e il prossimo riparte subito.
    Stop,
}

pub struct Gather<'a> {
    cache: &'a Mutex<TokenCache>,
    ctx: &'a ConnectorContext,
    budget: Budget,
    /// Messaggi gia' letti nel giro (`None` = da saltare), solo se un'altra regola li rivuole.
    messages: HashMap<String, Option<Arc<ParsedMessage>>>,
    files: HashMap<String, Arc<Files>>,
}

impl<'a> Gather<'a> {
    pub fn new(cache: &'a Mutex<TokenCache>, ctx: &'a ConnectorContext, started_ms: i64) -> Self {
        Self { cache, ctx, budget: Budget::new(started_ms), messages: HashMap::new(), files: HashMap::new() }
    }

    async fn message(&mut self, id: &str, keep: bool) -> Result<Option<Arc<ParsedMessage>>, ConnectorError> {
        if let Some(known) = self.messages.get(id) {
            return Ok(known.clone());
        }
        let parsed = match fetch::message(self.cache, self.ctx, id).await? {
            Fetched::Message(message) => Some(Arc::new(*message)),
            Fetched::Gone | Fetched::Unreadable => None,
        };
        if keep {
            self.messages.insert(id.to_string(), parsed.clone());
        }
        Ok(parsed)
    }

    /// I file del messaggio; `None` se il budget dei byte non ne ammette lo scarico.
    async fn files(&mut self, message: &ParsedMessage, keep: bool) -> Result<Option<Arc<Files>>, ConnectorError> {
        if let Some(known) = self.files.get(&message.id) {
            return Ok(Some(known.clone()));
        }
        let selection = select(&message.attachments);
        if !self.budget.admits(wanted_bytes(&message.attachments, &selection)) {
            return Ok(None);
        }
        let files = Arc::new(fetch::download(self.cache, self.ctx, message, selection).await?);
        self.budget.spend(files.bytes());
        if keep {
            self.files.insert(message.id.clone(), files.clone());
        }
        Ok(Some(files))
    }

    /// Elabora `id` per `route`. `keep`: un'altra regola dello stesso giro lo rivuole.
    pub async fn step(
        &mut self,
        cursor: &mut Cursor,
        route: &str,
        id: &str,
        keep: bool,
    ) -> Result<Step, ConnectorError> {
        let now = self.ctx.now();
        if self.budget.time_up(now) {
            return Ok(Step::Stop);
        }
        let Some(message) = self.message(id, keep).await? else {
            cursor.route_mut(route, now).remember(id, now);
            self.budget.message_done();
            return Ok(Step::Skipped);
        };
        if !cursor.route_mut(route, now).accepts(message.internal_ms) {
            cursor.route_mut(route, now).remember(id, message.internal_ms);
            self.budget.message_done();
            return Ok(Step::Skipped);
        }
        let Some(files) = self.files(&message, keep).await? else {
            return Ok(Step::Stop);
        };
        self.budget.message_done();
        Ok(Step::Item(Box::new(deliver(cursor.route_mut(route, now), route, &message, &files))))
    }
}

#[cfg(test)]
mod tests;
