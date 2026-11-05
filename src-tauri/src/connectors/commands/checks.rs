//! Le verifiche pure degli argomenti dei comandi: niente `AppHandle`, niente I/O,
//! niente orologio. Ogni funzione risponde con un testo italiano pronto per
//! l'utente; nessun errore riporta mai il valore di un segreto.

use crate::connectors::contracts::{Connector, ConnectorImpl, SourceConnector};
use crate::connectors::descriptor::validate::{normalize_values, validate_values};
use crate::connectors::descriptor::{ActionId, ConnectorDescriptor, HubCapability};
use crate::connectors::ids::{valid_instance_id, valid_link_id, valid_route_id};
use crate::connectors::limits::LABEL_MAX;
use crate::connectors::model::{label_ok, ConnectorRoute, RouteThread, RouteWake};
use crate::connectors::registry;
use crate::hub::{valid_ref_id, Channel};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

pub const NOT_AVAILABLE: &str = "Connettore non disponibile in questa versione dell'app.";
pub const INSTANCE_NOT_FOUND: &str = "Connettore configurato inesistente.";
pub const ROUTE_NOT_FOUND: &str = "Regola inesistente.";
pub const LINK_NOT_FOUND: &str = "Collegamento inesistente.";

pub fn check_instance_id(id: &str) -> Result<(), String> {
    if valid_instance_id(id) {
        Ok(())
    } else {
        Err("Identificativo del connettore non valido.".to_string())
    }
}

pub fn check_route_id(id: &str) -> Result<(), String> {
    if valid_route_id(id) {
        Ok(())
    } else {
        Err("Identificativo della regola non valido.".to_string())
    }
}

pub fn check_link_id(id: &str) -> Result<(), String> {
    if valid_link_id(id) {
        Ok(())
    } else {
        Err("Identificativo del collegamento non valido.".to_string())
    }
}

/// Il connettore compilato con questo id.
pub fn find_connector(id: &str) -> Result<ConnectorImpl, String> {
    registry::find(id).ok_or_else(|| NOT_AVAILABLE.to_string())
}

/// L'etichetta ripulita dagli spazi ai bordi.
pub fn check_label(raw: &str) -> Result<String, String> {
    let label = raw.trim();
    if label_ok(label) {
        Ok(label.to_string())
    } else {
        Err(format!("L'etichetta deve avere da 1 a {LABEL_MAX} caratteri e nessun carattere di controllo."))
    }
}

/// La configurazione: ripulita, valida contro i campi del descrittore (i campi
/// `secret` sono rifiutati: vanno nel portachiavi) e contro le regole del connettore.
pub fn check_config(connector: &dyn Connector, config: Value) -> Result<Value, String> {
    let fields = connector.descriptor().config_fields;
    let config = normalize_values(fields, config);
    validate_values(fields, &config).map_err(|e| format!("Configurazione non valida: {e}."))?;
    connector.validate_config(&config).map_err(|e| format!("Configurazione non valida: {e}."))?;
    Ok(config)
}

/// Lo slot dei segreti deve essere dichiarato dal descrittore.
pub fn check_slot(descriptor: &ConnectorDescriptor, slot: &str) -> Result<(), String> {
    if descriptor.declares_slot(slot) {
        Ok(())
    } else {
        Err("Segreto non previsto da questo connettore.".to_string())
    }
}

/// L'azione deve esistere ed essere dichiarata dal descrittore.
pub fn check_action(descriptor: &ConnectorDescriptor, raw: &str) -> Result<ActionId, String> {
    let action = ActionId::parse(raw).ok_or("Azione sconosciuta.")?;
    if descriptor.has_action(action) {
        Ok(action)
    } else {
        Err("Azione non prevista da questo connettore.".to_string())
    }
}

/// Le regole esistono solo per le fonti.
pub fn source_of(imp: ConnectorImpl) -> Result<&'static dyn SourceConnector, String> {
    match imp {
        ConnectorImpl::Source(source) => Ok(source),
        ConnectorImpl::Contact(_) => Err("Le regole sono solo per i connettori di tipo fonte.".to_string()),
    }
}

/// I campi di una regola come arrivano dal frontend (creazione e modifica).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, rename = "ConnectorRouteInput")]
pub struct RouteInput {
    pub label: String,
    #[ts(type = "Record<string, unknown>")]
    pub filter: Value,
    /// "general" o l'id di un workspace.
    pub channel: String,
    pub thread: RouteThread,
    /// "none", "owner" o "secretary:<nome>".
    pub wake: String,
    pub enabled: bool,
}

/// Una regola che ha superato le verifiche pure.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteDraft {
    pub label: String,
    pub filter: Value,
    pub channel: Channel,
    pub thread: RouteThread,
    pub wake: RouteWake,
    pub enabled: bool,
}

/// Le verifiche di una regola che non richiedono di guardare il mondo: etichetta,
/// canale e thread ben formati, risveglio ammesso dal descrittore, filtro valido
/// contro i campi del descrittore e contro `validate_filter` del connettore.
pub fn check_route(source: &dyn SourceConnector, input: RouteInput) -> Result<RouteDraft, String> {
    let descriptor = source.descriptor();
    let label = check_label(&input.label)?;
    let channel = Channel::parse(&input.channel).map_err(|_| "Canale non valido.".to_string())?;
    if let RouteThread::Existing { thread_id } = &input.thread {
        if !valid_ref_id(thread_id) {
            return Err("Identificativo del thread non valido.".to_string());
        }
    }
    let wake = RouteWake::parse(&input.wake).map_err(|e| format!("Risveglio non valido: {e}."))?;
    if wake.wakes() && !descriptor.has_capability(HubCapability::WakeSecretaries) {
        return Err("Questo connettore non può svegliare i segretari: scegli «nessuno».".to_string());
    }
    let filter = normalize_values(descriptor.filter_fields, input.filter);
    validate_values(descriptor.filter_fields, &filter).map_err(|e| format!("Filtro non valido: {e}."))?;
    source.validate_filter(&filter).map_err(|e| format!("Filtro non valido: {e}."))?;
    Ok(RouteDraft { label, filter, channel, thread: input.thread, wake, enabled: input.enabled })
}

/// Il thread da cercare nell'HUB prima di salvare, se la regola ne punta uno
/// esistente: sempre per una regola nuova, per una esistente solo se thread o
/// canale cambiano o se la si riabilita (cosi' si puo' rinominare una regola con l'HUB spento).
pub fn thread_to_verify<'a>(old: Option<&ConnectorRoute>, draft: &'a RouteDraft) -> Option<&'a str> {
    let RouteThread::Existing { thread_id } = &draft.thread else { return None };
    let unchanged = old.is_some_and(|old| {
        old.channel == draft.channel && old.thread == draft.thread && (old.enabled || !draft.enabled)
    });
    (!unchanged).then_some(thread_id.as_str())
}

/// Scrive la bozza in una regola esistente. Un errore di auto-disabilitazione
/// (thread sparito) si toglie quando la regola si riabilita o il suo bersaglio cambia.
pub fn apply_draft(route: &mut ConnectorRoute, draft: RouteDraft, now: i64) {
    let retargeted = route.channel != draft.channel || route.thread != draft.thread;
    if draft.enabled || retargeted {
        route.error = None;
    }
    route.label = draft.label;
    route.filter = draft.filter;
    route.channel = draft.channel;
    route.thread = draft.thread;
    route.wake = draft.wake;
    route.enabled = draft.enabled;
    route.updated_at = now;
}

/// Una regola nuova dalla bozza.
pub fn new_route(id: String, instance: &str, draft: RouteDraft, now: i64) -> ConnectorRoute {
    ConnectorRoute {
        id,
        instance: instance.to_string(),
        label: draft.label,
        filter: draft.filter,
        channel: draft.channel,
        thread: draft.thread,
        wake: draft.wake,
        enabled: draft.enabled,
        error: None,
        created_at: now,
        updated_at: now,
    }
}

/// I filtri di `connector_list_routes`, gia' validati.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteFilter {
    pub channel: Option<Channel>,
    pub thread_id: Option<String>,
}

pub fn parse_route_filter(channel: Option<&str>, thread_id: Option<&str>) -> Result<RouteFilter, String> {
    let channel = channel.map(Channel::parse).transpose().map_err(|_| "Canale non valido.".to_string())?;
    if thread_id.is_some_and(|id| !valid_ref_id(id)) {
        return Err("Identificativo del thread non valido.".to_string());
    }
    Ok(RouteFilter { channel, thread_id: thread_id.map(str::to_string) })
}

/// Le regole che passano i filtri: `thread_id` seleziona quelle che puntano a quel thread esistente.
pub fn filter_routes(routes: &[ConnectorRoute], filter: &RouteFilter) -> Vec<ConnectorRoute> {
    routes
        .iter()
        .filter(|r| filter.channel.as_ref().is_none_or(|c| r.channel == *c))
        .filter(|r| {
            filter
                .thread_id
                .as_deref()
                .is_none_or(|id| matches!(&r.thread, RouteThread::Existing { thread_id } if thread_id == id))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
