//! Conformita' dei connettori: ogni connettore registrato (e i due finti dei test)
//! deve avere un descrittore valido, permessi coerenti con il suo tipo, migrazioni
//! che riescono dalla versione 1 a quella corrente e non andare in panico su
//! configurazioni e filtri malformati, vuoti, enormi o unicode. Il test gira sui
//! connettori del registro di produzione e sui finti; ogni connettore aggiunto al
//! registro e' verificato da solo. Guida: docs/CONNECTORS.MD.

use super::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, Connector, ConnectorContext, ConnectorError, ConnectorImpl, PollBatch,
    SourceConnector,
};
use super::descriptor::validate::{validate_descriptor, validate_values};
use super::descriptor::{ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, FieldKind, HubCapability};
use super::limits::MIN_INTERVAL;
use super::{registry, testing};
use serde_json::{json, Value};
use std::collections::HashSet;

/// Un valore valido per ogni campo non segreto di `fields`: per un testo il suo segnaposto (e'
/// un esempio del formato atteso, cosi' passa anche la validazione semantica), altrimenti "x".
fn sample_values(fields: &[ConfigField]) -> Value {
    let mut map = serde_json::Map::new();
    for field in fields {
        let value = match &field.kind {
            FieldKind::Secret => continue,
            FieldKind::Text { placeholder, .. } => json!(if placeholder.is_empty() { "x" } else { *placeholder }),
            FieldKind::Number { min, .. } => json!(min),
            FieldKind::Bool { default } => json!(default),
            FieldKind::Select { options } => json!(options.first().map_or("", |o| o.value)),
            FieldKind::Workspace => json!("ws-1-1"),
            FieldKind::Thread => json!({"channel": "general", "thread_id": "thr-1-1"}),
        };
        map.insert(field.key.to_string(), value);
    }
    Value::Object(map)
}

/// Valori che nessun connettore deve far andare in panico.
fn hostile_values() -> Vec<Value> {
    let mut nested = json!(1);
    for _ in 0..100 {
        nested = json!({ "a": [nested] });
    }
    vec![
        Value::Null,
        json!([]),
        json!("testo"),
        json!(-1),
        json!(1.5e300),
        json!({}),
        json!({"a": null}),
        json!({"": ""}),
        json!({"ünï\u{0}\u{202e}😀": "ünï\u{0}\u{202e}😀 \u{1F468}\u{200D}\u{1F469}"}),
        json!({"query": "x".repeat(2_000_000), "mailbox": "y".repeat(2_000_000)}),
        json!({"interval_secs": u64::MAX, "interval_minutes": u64::MAX, "count": i64::MIN}),
        nested,
    ]
}

/// Tutti i problemi di conformita' di `connector` (vuoto = conforme).
pub(super) fn problems(connector: ConnectorImpl) -> Vec<String> {
    let d = connector.descriptor();
    let mut found: Vec<String> = validate_descriptor(d).into_iter().map(|p| format!("descrittore: {p}")).collect();
    if connector.kind() != d.kind {
        found.push("il tipo del descrittore non coincide con il trait implementato".to_string());
    }
    if d.kind == ConnectorKind::Source && !d.has_capability(HubCapability::DeliverMessages) {
        found.push("una fonte deve dichiarare deliver_messages".to_string());
    }
    for field in d.config_fields.iter().filter(|f| f.key == "interval_secs") {
        match field.kind {
            FieldKind::Number { min, .. } if min >= MIN_INTERVAL.as_secs() as i64 => {}
            _ => found.push("interval_secs deve essere un numero con minimo >= 60".to_string()),
        }
    }
    for field in d.config_fields.iter().filter(|f| f.key == "interval_minutes") {
        match field.kind {
            FieldKind::Number { min, .. } if min >= 1 => {}
            _ => found.push("interval_minutes deve essere un numero con minimo >= 1".to_string()),
        }
    }
    found.extend(migration_problems(connector.connector(), d));
    found.extend(sample_problems(connector, d));
    for value in hostile_values() {
        hostile_input(connector, d, &value);
    }
    found
}

fn migration_problems(connector: &dyn Connector, d: &ConnectorDescriptor) -> Vec<String> {
    let mut found = Vec::new();
    let mut config = sample_values(d.config_fields);
    for from in 1..d.version {
        match connector.migrate_config(from, config.clone()) {
            Ok(next) if next.is_object() => config = next,
            Ok(_) => found.push(format!("migrazione {from} -> {}: il risultato non e' un oggetto", from + 1)),
            Err(e) => found.push(format!("migrazione {from} -> {}: {e}", from + 1)),
        }
    }
    found
}

fn sample_problems(connector: ConnectorImpl, d: &ConnectorDescriptor) -> Vec<String> {
    let mut found = Vec::new();
    let config = sample_values(d.config_fields);
    if let Err(e) = validate_values(d.config_fields, &config) {
        found.push(format!("la configurazione di esempio non passa la validazione generica: {e}"));
    }
    if let ConnectorImpl::Source(source) = connector {
        let filter = sample_values(d.filter_fields);
        if let Err(e) = validate_values(d.filter_fields, &filter) {
            found.push(format!("il filtro di esempio non passa la validazione generica: {e}"));
        }
        if source.validate_filter(&filter).is_err() {
            found.push("validate_filter rifiuta il filtro di esempio".to_string());
        }
    }
    if connector.connector().validate_config(&config).is_err() {
        found.push("validate_config rifiuta la configurazione di esempio".to_string());
    }
    found
}

/// Chiama ogni funzione pura con un input ostile: nessun panico (l'esito e' libero).
fn hostile_input(connector: ConnectorImpl, d: &ConnectorDescriptor, value: &Value) {
    let c = connector.connector();
    let _ = c.validate_config(value);
    for from in 0..=d.version + 1 {
        let _ = c.migrate_config(from, value.clone());
    }
    let _ = validate_values(d.config_fields, value);
    let _ = validate_values(d.filter_fields, value);
    if let ConnectorImpl::Source(source) = connector {
        let _ = source.validate_filter(value);
    }
}

fn duplicates(list: &[ConnectorImpl]) -> Vec<&'static str> {
    let mut seen = HashSet::new();
    list.iter().map(|c| c.descriptor().id).filter(|id| !seen.insert(*id)).collect()
}

#[test]
fn every_registered_connector_conforms() {
    let list = registry::all();
    assert!(duplicates(list).is_empty(), "id duplicati nel registro: {:?}", duplicates(list));
    for connector in list {
        let found = problems(*connector);
        assert!(found.is_empty(), "{} non e' conforme:\n  {}", connector.descriptor().id, found.join("\n  "));
    }
}

#[test]
fn the_fake_connectors_conform() {
    let fakes = testing::fakes();
    assert!(duplicates(&fakes).is_empty());
    for connector in fakes {
        let found = problems(connector);
        assert!(found.is_empty(), "{} non e' conforme:\n  {}", connector.descriptor().id, found.join("\n  "));
    }
}

#[test]
fn the_registered_fakes_are_seen_through_the_registry_override() {
    let _registered = registry::test_registry::Registered::new(testing::fakes());
    assert_eq!(registry::all().len(), 2);
    for connector in registry::all() {
        assert!(problems(*connector).is_empty());
    }
}

// ---- il test della conformita' prende davvero le violazioni ----

/// Un connettore `source` costruito per violare una regola.
struct Custom {
    descriptor: &'static ConnectorDescriptor,
    migrate: fn(u32, Value) -> Result<Value, String>,
}

impl Connector for Custom {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        self.descriptor
    }

    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String> {
        (self.migrate)(from, config)
    }

    fn validate_config(&self, _config: &Value) -> Result<(), String> {
        Ok(())
    }

    fn action<'a>(
        &'a self,
        _: &'a ConnectorContext,
        _: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(async { Err(ConnectorError::Fatal("no".to_string())) })
    }
}

impl SourceConnector for Custom {
    fn validate_filter(&self, _filter: &Value) -> Result<(), String> {
        Ok(())
    }

    fn poll<'a>(
        &'a self,
        _: &'a ConnectorContext,
        _: &'a [ActiveRoute],
        _: Option<Value>,
    ) -> BoxFuture<'a, Result<PollBatch, ConnectorError>> {
        Box::pin(async { Err(ConnectorError::Fatal("no".to_string())) })
    }
}

fn leak(descriptor: ConnectorDescriptor, migrate: fn(u32, Value) -> Result<Value, String>) -> ConnectorImpl {
    let custom: &'static Custom = Box::leak(Box::new(Custom { descriptor: Box::leak(Box::new(descriptor)), migrate }));
    ConnectorImpl::Source(custom)
}

fn identity(_: u32, config: Value) -> Result<Value, String> {
    Ok(config)
}

#[test]
fn a_source_with_contact_permissions_is_not_conformant() {
    use crate::connectors::descriptor::Permissions;
    let bad = Permissions {
        hosts: &["api.fake-source.example"],
        hub: &[HubCapability::DeliverMessages, HubCapability::ReceiveNotices, HubCapability::SendAsUser],
    };
    let found = problems(leak(ConnectorDescriptor { permissions: bad, ..testing::FAKE_SOURCE }, identity));
    let text = found.join(" | ");
    assert!(text.contains("receive_notices") && text.contains("send_as_user"), "{text}");
}

#[test]
fn a_contact_with_source_permissions_is_not_conformant() {
    use crate::connectors::descriptor::Permissions;
    let bad = Permissions {
        hosts: &["api.fake-contact.example"],
        hub: &[HubCapability::DeliverMessages, HubCapability::WakeSecretaries],
    };
    let descriptor = ConnectorDescriptor { permissions: bad, ..testing::FAKE_CONTACT };
    let text = validate_descriptor(&descriptor).join(" | ");
    assert!(text.contains("deliver_messages") && text.contains("wake_secretaries"), "{text}");
}

#[test]
fn a_failing_migration_a_wrong_kind_and_bad_hosts_are_reported() {
    use crate::connectors::descriptor::Permissions;
    let failing = |from: u32, _| Err(format!("rotta dalla {from}"));
    let text = problems(leak(testing::FAKE_SOURCE, failing)).join(" | ");
    assert!(text.contains("migrazione 1 -> 2: rotta dalla 1"), "{text}");

    let text = problems(leak(ConnectorDescriptor { kind: ConnectorKind::Contact, ..testing::FAKE_SOURCE }, identity))
        .join(" | ");
    assert!(text.contains("non coincide"), "{text}");

    let hosts = Permissions { hosts: &["HTTPS://Evil.com"], hub: testing::FAKE_SOURCE.permissions.hub };
    let text = problems(leak(ConnectorDescriptor { permissions: hosts, ..testing::FAKE_SOURCE }, identity)).join(" | ");
    assert!(text.contains("host"), "{text}");

    let no_deliver = Permissions { hosts: &[], hub: &[] };
    let text =
        problems(leak(ConnectorDescriptor { permissions: no_deliver, ..testing::FAKE_SOURCE }, identity)).join(" | ");
    assert!(text.contains("deliver_messages"), "{text}");
}

#[test]
fn duplicate_ids_and_a_fast_polling_interval_are_reported() {
    let twice = vec![testing::fakes()[0], testing::fakes()[0]];
    assert_eq!(duplicates(&twice), vec!["fake-source"]);
    static FAST: [ConfigField; 1] = [ConfigField {
        key: "interval_secs",
        label: "Intervallo",
        help: "",
        required: false,
        kind: FieldKind::Number { min: 10, max: 100 },
    }];
    let text =
        problems(leak(ConnectorDescriptor { config_fields: &FAST, ..testing::FAKE_SOURCE }, identity)).join(" | ");
    assert!(text.contains("interval_secs"), "{text}");
}

#[test]
fn hostile_values_do_not_panic_and_the_sample_values_are_valid() {
    for connector in testing::fakes() {
        let d = connector.descriptor();
        assert!(validate_values(d.config_fields, &sample_values(d.config_fields)).is_ok());
    }
    assert!(hostile_values().len() >= 10);
}

#[test]
fn a_zero_minutes_polling_interval_is_reported() {
    static ZERO: [ConfigField; 1] = [ConfigField {
        key: "interval_minutes",
        label: "Intervallo",
        help: "",
        required: false,
        kind: FieldKind::Number { min: 0, max: 60 },
    }];
    let text =
        problems(leak(ConnectorDescriptor { config_fields: &ZERO, ..testing::FAKE_SOURCE }, identity)).join(" | ");
    assert!(text.contains("interval_minutes"), "{text}");
}
