//! Il connettore come lo vede il registro: tipo, conformita', migrazioni e filtro delle notifiche.

use super::*;
use crate::connectors::contracts::ConnectorImpl;
use crate::connectors::{conformance, registry};
use serde_json::json;

#[test]
fn the_connector_conforms_to_the_contract() {
    let found = conformance::problems(ConnectorImpl::Contact(&TELEGRAM));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn it_is_the_registered_contact_connector() {
    let registered = registry::find("telegram").expect("registrato");
    assert!(matches!(registered, ConnectorImpl::Contact(_)));
    assert_eq!(registered.descriptor().id, TELEGRAM.descriptor().id);
}

#[test]
fn there_are_no_migrations_at_version_one() {
    assert_eq!(TELEGRAM.descriptor().version, 1);
    for from in [0, 1, 2, u32::MAX] {
        assert!(TELEGRAM.migrate_config(from, json!({})).is_err(), "{from}");
    }
}

#[test]
fn the_semantic_validation_accepts_the_generic_shape() {
    assert!(TELEGRAM.validate_config(&json!({"notify_done": true})).is_ok());
    assert!(TELEGRAM.validate_config(&json!(null)).is_ok());
}

#[test]
fn notices_follow_the_configuration() {
    let config = json!({"notify_failures": false, "notify_done": true});
    assert!(!TELEGRAM.wants_notice(&config, NoticeCategory::TaskFailed));
    assert!(TELEGRAM.wants_notice(&config, NoticeCategory::TaskDone));
    assert!(TELEGRAM.wants_notice(&config, NoticeCategory::Reply));
    assert!(!TELEGRAM.wants_notice(&config, NoticeCategory::External));
}
