use super::*;
use crate::connectors::descriptor::validate::{validate_descriptor, validate_values};
use serde_json::json;

#[test]
fn the_descriptor_is_valid() {
    assert!(validate_descriptor(&DESCRIPTOR).is_empty(), "{:?}", validate_descriptor(&DESCRIPTOR));
    assert_eq!(DESCRIPTOR.id, "telegram");
    assert_eq!(DESCRIPTOR.kind, ConnectorKind::Contact);
    assert_eq!(DESCRIPTOR.permissions.hosts, &["api.telegram.org"]);
}

#[test]
fn the_default_configuration_passes_the_generic_validation() {
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({"notify_done": true})).is_ok());
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({})).is_ok(), "il token sta nel portachiavi");
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({"bot_token": "x"})).is_err(), "mai in config");
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({"notify_done": "si"})).is_err());
}

#[test]
fn defaults_apply_when_the_switch_is_missing() {
    let empty = json!({});
    for category in [
        NoticeCategory::Reply,
        NoticeCategory::Question,
        NoticeCategory::TaskFailed,
        NoticeCategory::TaskAttention,
        NoticeCategory::ProposalCreated,
        NoticeCategory::PlanWithoutProposal,
        NoticeCategory::ThreadConcluded,
    ] {
        assert!(notice_wanted(&empty, category), "{category:?}");
    }
    assert!(!notice_wanted(&empty, NoticeCategory::TaskDone));
    assert!(!notice_wanted(&empty, NoticeCategory::ReviewDone));
}

#[test]
fn an_explicit_switch_wins_over_the_default() {
    let config = json!({"notify_replies": false, "notify_done": true});
    assert!(!notice_wanted(&config, NoticeCategory::Reply));
    assert!(notice_wanted(&config, NoticeCategory::TaskDone));
    assert!(notice_wanted(&config, NoticeCategory::ReviewDone));
}

#[test]
fn a_non_boolean_value_falls_back_to_the_default() {
    let config = json!({"notify_replies": "no", "notify_done": 1});
    assert!(notice_wanted(&config, NoticeCategory::Reply));
    assert!(!notice_wanted(&config, NoticeCategory::TaskDone));
    assert!(!notice_wanted(&json!(null), NoticeCategory::TaskDone));
}

#[test]
fn external_items_are_never_forwarded() {
    assert!(!notice_wanted(&json!({}), NoticeCategory::External));
    assert!(!notice_wanted(&json!({"notify_external": true}), NoticeCategory::External));
}

#[test]
fn every_notice_key_is_a_declared_switch() {
    for category in [
        NoticeCategory::Reply,
        NoticeCategory::Question,
        NoticeCategory::TaskFailed,
        NoticeCategory::TaskAttention,
        NoticeCategory::ProposalCreated,
        NoticeCategory::PlanWithoutProposal,
        NoticeCategory::ThreadConcluded,
        NoticeCategory::ReviewDone,
        NoticeCategory::TaskDone,
    ] {
        let key = notice_key(category).expect("inoltrata");
        assert!(matches!(DESCRIPTOR.config_field(key).map(|f| f.kind), Some(FieldKind::Bool { .. })), "{key}");
    }
}

#[test]
fn mirroring_app_messages_is_on_by_default_and_follows_the_switch() {
    assert!(mirror_wanted(&json!({})));
    assert!(mirror_wanted(&json!(null)));
    assert!(mirror_wanted(&json!({"mirror_app_messages": true})));
    assert!(!mirror_wanted(&json!({"mirror_app_messages": false})));
    assert!(mirror_wanted(&json!({"mirror_app_messages": "no"})), "un valore non booleano usa il predefinito");
}

#[test]
fn the_mirror_key_is_a_declared_switch_on_by_default() {
    assert!(matches!(
        DESCRIPTOR.config_field("mirror_app_messages").map(|f| f.kind),
        Some(FieldKind::Bool { default: true })
    ));
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({"mirror_app_messages": false})).is_ok());
    assert!(validate_values(DESCRIPTOR.config_fields, &json!({"mirror_app_messages": "no"})).is_err());
}
