//! Un giro di una fonte (docs/CONNECTORS.MD §5/§7): `poll` con il cursore, al piu'
//! `ITEMS_PER_ROUND` elementi nuovi, consegna uno per uno nell'HUB, e SOLO a
//! consegne riuscite il cursore nuovo entra nello stato (il supervisore poi lo
//! salva su disco). Garanzia "almeno una volta" con dedupe: un elemento gia'
//! consegnato (`seen`, per regola + id) si salta, e anche se il processo muore
//! prima del salvataggio l'HUB scarta il duplicato (stessa origine). Qualunque
//! errore di consegna ferma il giro: il cursore resta quello vecchio.
//!
//! Il thread di destinazione: esistente (se sparito la regola si disabilita con
//! `THREAD_GONE` e il giro riparte subito senza perdere gli altri elementi),
//! nuovo per elemento, o per conversazione (la mappa chiave -> thread dello
//! stato; una voce che punta a un thread sparito si scarta e se ne apre uno nuovo).
//! Un solo risveglio per thread per giro, sull'ULTIMO elemento che lo riceve.
//!
//! Niente `AppHandle`: tutto passa da `HubPort`, cosi' si prova con i finti.

use crate::connectors::contracts::{
    ActiveRoute, ConnectorContext, ConnectorError, HubPort, SourceConnector, SourceItem, StateHandle,
};
use crate::connectors::descriptor::HubCapability;
use crate::connectors::limits::POLL_TIMEOUT;
use crate::connectors::model::{ConnectorRoute, RouteThread};
use crate::connectors::runtime::time::with_timeout;
use crate::connectors::state::{conversation_key, MAIN_CURSOR};
use crate::hub::ingress::{DeliveryTarget, ExternalDelivery, ExternalWake};
use crate::hub::{Author, MessageOrigin};
use std::collections::{HashMap, HashSet};

/// Tutto cio' che serve a un giro.
pub struct RoundCtx<'a> {
    pub source: &'a dyn SourceConnector,
    pub ctx: &'a ConnectorContext,
    pub hub: &'a dyn HubPort,
    pub state: &'a StateHandle,
    /// Le regole dell'istanza (tutte: le non attive servono a pulire lo stato).
    pub routes: &'a [ConnectorRoute],
    /// Elementi da consegnare al massimo in questo giro.
    pub max_items: usize,
}

/// Come finisce un giro.
#[derive(Debug, PartialEq)]
pub enum RoundEnd {
    /// Giro completo: stato da salvare. `more`: il prossimo parte subito.
    Done {
        more: bool,
    },
    /// Una regola e' stata disabilitata: rileggere le regole e rifare il giro subito.
    Retry,
    Failed(ConnectorError),
}

#[derive(Debug, PartialEq)]
pub struct RoundResult {
    pub end: RoundEnd,
    /// Regole da disabilitare con `THREAD_GONE` (il supervisore le salva).
    pub disable: Vec<String>,
}

impl RoundResult {
    fn failed(error: ConnectorError, disable: Vec<String>) -> Self {
        Self { end: RoundEnd::Failed(error), disable }
    }
}

enum Resolved {
    /// La destinazione e, se e' un thread nuovo di una conversazione, la chiave da registrare.
    Target(DeliveryTarget, Option<String>),
    Gone,
}

/// Con chi un elemento condivide il thread (per un solo risveglio).
fn thread_group(route: &ConnectorRoute, item: &SourceItem, index: usize) -> String {
    match (&route.thread, &item.conversation_key) {
        (RouteThread::Existing { thread_id }, _) => format!("t:{}:{thread_id}", route.channel.as_str()),
        (RouteThread::PerConversation, Some(key)) => format!("c:{}:{key}", route.id),
        _ => format!("i:{index}"),
    }
}

pub async fn run_round(rc: &RoundCtx<'_>) -> RoundResult {
    let active: Vec<&ConnectorRoute> = rc.routes.iter().filter(|r| r.enabled).collect();
    {
        let live: HashSet<String> = rc.routes.iter().map(|r| r.id.clone()).collect();
        rc.state.lock().prune_routes(&live);
    }
    if active.is_empty() {
        return RoundResult { end: RoundEnd::Done { more: false }, disable: Vec::new() };
    }
    let for_poll: Vec<ActiveRoute> =
        active.iter().map(|r| ActiveRoute { id: r.id.clone(), filter: r.filter.clone() }).collect();
    let cursor = rc.state.lock().cursor(MAIN_CURSOR).cloned();
    let batch = match with_timeout(POLL_TIMEOUT, rc.source.poll(rc.ctx, &for_poll, cursor)).await {
        Ok(batch) => batch,
        Err(e) => return RoundResult::failed(e, Vec::new()),
    };

    // Gli elementi da consegnare: non visti, di una regola attiva, al piu' `max_items`.
    let mut todo: Vec<(&ConnectorRoute, &SourceItem)> = Vec::new();
    let mut planned = HashSet::new();
    let mut leftover = false;
    for item in &batch.items {
        let Some(route) = active.iter().find(|r| r.id == item.route) else { continue };
        let seen = rc.state.lock().is_seen(&item.route, &item.external_id);
        if seen || !planned.insert((item.route.as_str(), item.external_id.as_str())) {
            continue;
        }
        if todo.len() >= rc.max_items {
            leftover = true;
            break;
        }
        todo.push((route, item));
    }
    let mut last_of_group: HashMap<String, usize> = HashMap::new();
    for (index, (route, item)) in todo.iter().enumerate() {
        last_of_group.insert(thread_group(route, item, index), index);
    }
    let may_wake = rc.ctx.descriptor().has_capability(HubCapability::WakeSecretaries);

    let mut disable = Vec::new();
    for (index, (route, item)) in todo.iter().enumerate() {
        let (target, record) = match resolve(rc, route, item).await {
            Ok(Resolved::Target(target, record)) => (target, record),
            Ok(Resolved::Gone) => {
                disable.push(route.id.clone());
                return RoundResult { end: RoundEnd::Retry, disable };
            }
            Err(e) => return RoundResult::failed(e, disable),
        };
        let is_last = last_of_group.get(&thread_group(route, item, index)) == Some(&index);
        let wake = if is_last && may_wake { route.wake.to_external() } else { ExternalWake::None };
        let connector = rc.ctx.descriptor().id.to_string();
        let instance = rc.ctx.instance().to_string();
        let delivery = ExternalDelivery {
            channel: route.channel.clone(),
            target,
            author: Author::External {
                connector: connector.clone(),
                instance: instance.clone(),
                name: item.sender.clone(),
            },
            body: item.body.clone(),
            files: item.files.clone(),
            origin: MessageOrigin { connector, instance, external_id: item.external_id.clone() },
            wake,
        };
        match rc.hub.deliver_external(delivery).await {
            Ok(outcome) => {
                let mut state = rc.state.lock();
                state.mark_seen(&item.route, &item.external_id);
                if let Some(key) = record {
                    state.conversations.insert(&key, &outcome.thread_id);
                }
            }
            Err(message) => return RoundResult::failed(ConnectorError::Transient(message), disable),
        }
    }

    // Tutto consegnato: ora il cursore. Con elementi avanzati resta il vecchio:
    // il giro dopo ripete la lettura e i gia' consegnati si saltano.
    if !leftover {
        rc.state.lock().set_cursor(MAIN_CURSOR, batch.cursor);
    }
    RoundResult { end: RoundEnd::Done { more: leftover || batch.more }, disable }
}

async fn thread_exists(rc: &RoundCtx<'_>, route: &ConnectorRoute, thread_id: &str) -> Result<bool, ConnectorError> {
    rc.hub.thread_exists(&route.channel, thread_id).await.map_err(ConnectorError::Transient)
}

async fn resolve(rc: &RoundCtx<'_>, route: &ConnectorRoute, item: &SourceItem) -> Result<Resolved, ConnectorError> {
    let new_thread = || DeliveryTarget::New { title: item.title.clone() };
    match (&route.thread, &item.conversation_key) {
        (RouteThread::Existing { thread_id }, _) => Ok(if thread_exists(rc, route, thread_id).await? {
            Resolved::Target(DeliveryTarget::Existing(thread_id.clone()), None)
        } else {
            Resolved::Gone
        }),
        (RouteThread::PerConversation, Some(key)) => {
            let key = conversation_key(&route.id, key);
            let mapped = rc.state.lock().conversations.touch(&key).map(str::to_string);
            match mapped {
                Some(thread_id) if thread_exists(rc, route, &thread_id).await? => {
                    Ok(Resolved::Target(DeliveryTarget::Existing(thread_id), None))
                }
                Some(_) => {
                    rc.state.lock().conversations.remove(&key);
                    Ok(Resolved::Target(new_thread(), Some(key)))
                }
                None => Ok(Resolved::Target(new_thread(), Some(key))),
            }
        }
        _ => Ok(Resolved::Target(new_thread(), None)),
    }
}

#[cfg(test)]
mod tests;
