//! La validazione del descrittore stesso: serve ai test di conformita' (un connettore
//! e' compilato nell'app con un descrittore fisso, non cambia a runtime).

use super::{valid_connector_id, valid_key, ID_MAX, KEY_MAX};
use crate::connectors::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, FieldKind, HubCapability, Permissions,
};
use crate::connectors::limits;
use std::collections::HashSet;

/// Lunghezza massima di un testo d'interfaccia del descrittore (etichetta, aiuto).
const UI_TEXT_MAX: usize = 1000;
/// Lunghezza massima di un testo di un campo `text`.
const TEXT_FIELD_MAX: usize = 2000;
/// Lunghezza massima di un host (RFC 1035).
const HOST_MAX: usize = 253;
const HOST_LABEL_MAX: usize = 63;

/// Un nome di host minuscolo con almeno due etichette (`api.telegram.org`), non
/// un indirizzo IP: schema, porta, percorso e jolly non sono ammessi.
pub fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > HOST_MAX {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let label_ok = |l: &&str| {
        !l.is_empty()
            && l.len() <= HOST_LABEL_MAX
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    let top_is_numeric = labels.last().is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()));
    labels.len() >= 2 && labels.iter().all(label_ok) && !top_is_numeric
}

/// I problemi del descrittore (vuoto = valido).
pub fn validate_descriptor(d: &ConnectorDescriptor) -> Vec<String> {
    let mut problems = Vec::new();
    if !valid_connector_id(d.id) {
        problems.push(format!("id {:?} non valido ([a-z0-9-], senza trattini ai bordi, al piu' {ID_MAX})", d.id));
    }
    if d.version == 0 {
        problems.push("version deve essere >= 1".to_string());
    }
    for (name, text) in [("label", d.label), ("description", d.description)] {
        if text.trim().is_empty() || text.chars().count() > UI_TEXT_MAX {
            problems.push(format!("{name} vuota o oltre {UI_TEXT_MAX} caratteri"));
        }
    }
    problems.extend(field_problems("config_fields", d.config_fields, true));
    problems.extend(field_problems("filter_fields", d.filter_fields, false));
    if d.kind == ConnectorKind::Contact && !d.filter_fields.is_empty() {
        problems.push("un connettore contact non ha filter_fields".to_string());
    }
    problems.extend(action_problems(d.kind, d.actions));
    problems.extend(permission_problems(d.kind, &d.permissions));
    problems.extend(slot_problems(d));
    if d.limits.items_per_round.is_some_and(|n| n == 0 || n > limits::ITEMS_PER_ROUND) {
        problems.push(format!("limits.items_per_round deve essere in 1..={}", limits::ITEMS_PER_ROUND));
    }
    if d.limits.min_interval_secs.is_some_and(|s| s < limits::MIN_INTERVAL.as_secs()) {
        problems.push(format!("limits.min_interval_secs non puo' scendere sotto {}", limits::MIN_INTERVAL.as_secs()));
    }
    let min_default = limits::MIN_INTERVAL.as_secs().max(d.limits.min_interval_secs.unwrap_or(0));
    if d.limits.default_interval_secs.is_some_and(|s| s < min_default) {
        problems.push(format!(
            "limits.default_interval_secs non puo' scendere sotto {min_default} (il minimo del runtime e del descrittore)"
        ));
    }
    problems
}

fn field_problems(list: &str, fields: &[ConfigField], secrets_allowed: bool) -> Vec<String> {
    let mut problems = Vec::new();
    let mut keys = HashSet::new();
    for field in fields {
        let key = field.key;
        if !valid_key(key) {
            problems.push(format!("{list}: chiave {key:?} non valida ([a-z0-9_], al piu' {KEY_MAX})"));
        }
        if !keys.insert(key) {
            problems.push(format!("{list}: chiave {key:?} duplicata"));
        }
        for (name, text) in [("label", field.label), ("help", field.help)] {
            let too_long = text.chars().count() > UI_TEXT_MAX;
            if too_long || (name == "label" && text.trim().is_empty()) {
                problems.push(format!("{list}.{key}: {name} vuota o oltre {UI_TEXT_MAX} caratteri"));
            }
        }
        problems.extend(kind_problems(list, key, &field.kind, secrets_allowed));
    }
    problems
}

fn kind_problems(list: &str, key: &str, kind: &FieldKind, secrets_allowed: bool) -> Vec<String> {
    let mut problems = Vec::new();
    match kind {
        FieldKind::Text { max_len, .. } if *max_len == 0 || *max_len > TEXT_FIELD_MAX => {
            problems.push(format!("{list}.{key}: max_len deve essere in 1..={TEXT_FIELD_MAX}"));
        }
        FieldKind::Secret if !secrets_allowed => {
            problems.push(format!("{list}.{key}: un campo secret non puo' stare nel filtro"));
        }
        FieldKind::Number { min, max } if min > max => problems.push(format!("{list}.{key}: min > max")),
        FieldKind::Select { options } => {
            if options.is_empty() {
                problems.push(format!("{list}.{key}: select senza opzioni"));
            }
            let mut values = HashSet::new();
            for option in *options {
                if option.value.is_empty() || option.label.trim().is_empty() || !values.insert(option.value) {
                    problems.push(format!("{list}.{key}: opzione vuota o duplicata"));
                }
            }
        }
        _ => {}
    }
    problems
}

fn action_problems(kind: ConnectorKind, actions: &[Action]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids = HashSet::new();
    for action in actions {
        if !ids.insert(action.id) {
            problems.push(format!("azione {:?} duplicata", action.id.as_str()));
        }
        if action.label.trim().is_empty() {
            problems.push(format!("azione {:?} senza etichetta", action.id.as_str()));
        }
        if action.id == ActionId::Pair && kind != ConnectorKind::Contact {
            problems.push("l'azione pair e' solo dei connettori contact".to_string());
        }
    }
    problems
}

/// Le capacita' devono essere coerenti con il tipo: una fonte non riceve notifiche
/// ne' scrive come utente, un canale di contatto non consegna elementi esterni ne'
/// sveglia segretari per una regola.
fn permission_problems(kind: ConnectorKind, permissions: &Permissions) -> Vec<String> {
    let mut problems = Vec::new();
    let mut hosts = HashSet::new();
    for host in permissions.hosts {
        if !valid_host(host) {
            problems.push(format!("host {host:?} non valido (nome minuscolo, senza schema, porta o percorso)"));
        }
        if !hosts.insert(*host) {
            problems.push(format!("host {host:?} duplicato"));
        }
    }
    let mut capabilities = HashSet::new();
    for capability in permissions.hub {
        if !capabilities.insert(*capability) {
            problems.push(format!("capacita' {} duplicata", capability.as_str()));
        }
        let allowed = match kind {
            ConnectorKind::Source => {
                matches!(capability, HubCapability::DeliverMessages | HubCapability::WakeSecretaries)
            }
            ConnectorKind::Contact => matches!(capability, HubCapability::ReceiveNotices | HubCapability::SendAsUser),
        };
        if !allowed {
            problems.push(format!("la capacita' {} non e' ammessa per questo tipo di connettore", capability.as_str()));
        }
    }
    problems
}

fn slot_problems(d: &ConnectorDescriptor) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: HashSet<&str> =
        d.config_fields.iter().filter(|f| matches!(f.kind, FieldKind::Secret)).map(|f| f.key).collect();
    for slot in d.internal_secret_slots {
        if !valid_key(slot) {
            problems.push(format!("slot interno {slot:?} non valido ([a-z0-9_], al piu' {KEY_MAX})"));
        }
        if !seen.insert(slot) {
            problems.push(format!("slot interno {slot:?} duplicato o uguale a un campo secret"));
        }
    }
    problems
}
