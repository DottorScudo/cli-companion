//! Il connettore come lo vede il registro: tipo, conformita', migrazioni e validazioni.

use super::*;
use crate::connectors::contracts::ConnectorImpl;
use crate::connectors::runtime::supervisor::source_interval;
use crate::connectors::{conformance, registry};
use serde_json::json;
use std::time::Duration;

#[test]
fn the_connector_conforms_to_the_contract() {
    let found = conformance::problems(ConnectorImpl::Source(&GMAIL));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn it_is_the_registered_source_connector() {
    let registered = registry::find("gmail").expect("registrato");
    assert!(matches!(registered, ConnectorImpl::Source(_)));
    assert_eq!(registered.descriptor().id, GMAIL.descriptor().id);
}

#[test]
fn there_are_no_migrations_at_version_one() {
    assert_eq!(GMAIL.descriptor().version, 1);
    for from in [0, 1, 2, u32::MAX] {
        assert!(GMAIL.migrate_config(from, json!({})).is_err(), "{from}");
    }
}

#[test]
fn the_samples_pass_the_semantic_validation() {
    let d = GMAIL.descriptor();
    assert!(GMAIL.validate_config(&json!({"client_id": "1234567890-abc.apps.googleusercontent.com"})).is_ok());
    assert!(GMAIL.validate_config(&json!({"client_id": "x"})).is_err());
    assert!(GMAIL.validate_config(&json!({})).is_err());
    assert!(GMAIL.validate_filter(&json!({"query": "from:mario@gmail.com"})).is_ok());
    assert!(GMAIL.validate_filter(&json!({"query": "(da:mario"})).is_err());
    let placeholder = |fields: &[crate::connectors::descriptor::ConfigField]| match fields.first().map(|f| f.kind) {
        Some(crate::connectors::descriptor::FieldKind::Text { placeholder, .. }) => placeholder,
        _ => "",
    };
    assert!(GMAIL.validate_config(&json!({"client_id": placeholder(d.config_fields)})).is_ok());
    assert!(GMAIL.validate_filter(&json!({"query": placeholder(d.filter_fields)})).is_ok());
}

#[test]
fn the_interval_defaults_to_two_minutes_and_follows_the_configuration() {
    let d = GMAIL.descriptor();
    assert_eq!(source_interval(d, &json!({})), Duration::from_secs(120));
    assert_eq!(source_interval(d, &json!({"interval_minutes": 5})), Duration::from_secs(300));
    assert_eq!(source_interval(d, &json!({"interval_minutes": 1})), Duration::from_secs(60));
}

#[test]
fn the_descriptor_declares_the_three_google_hosts_and_the_refresh_slot() {
    let d = GMAIL.descriptor();
    assert_eq!(d.permissions.hosts, &["accounts.google.com", "oauth2.googleapis.com", "gmail.googleapis.com"]);
    assert!(d.declares_slot("refresh_token") && d.declares_slot("client_secret"));
}
