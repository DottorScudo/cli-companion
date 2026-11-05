use super::validate::*;
use super::*;
use crate::connectors::testing::{FAKE_CONTACT, FAKE_SOURCE};
use serde_json::json;

const OPTIONS: &[SelectOption] = &[SelectOption { value: "a", label: "A" }, SelectOption { value: "b", label: "B" }];

fn field(key: &'static str, required: bool, kind: FieldKind) -> ConfigField {
    ConfigField { key, label: "Etichetta", help: "", required, kind }
}

fn text(max_len: usize) -> FieldKind {
    FieldKind::Text { max_len, placeholder: "" }
}

#[test]
fn the_fake_descriptors_are_valid() {
    assert_eq!(validate_descriptor(&FAKE_SOURCE), Vec::<String>::new());
    assert_eq!(validate_descriptor(&FAKE_CONTACT), Vec::<String>::new());
}

fn problems_of(d: ConnectorDescriptor) -> String {
    validate_descriptor(&d).join(" | ")
}

#[test]
fn invalid_ids_versions_and_texts_are_reported() {
    let too_long: &'static str = Box::leak("a".repeat(33).into_boxed_str());
    for id in ["", "-a", "a-", "A", "a_b", "a b", "à", too_long] {
        assert!(problems_of(ConnectorDescriptor { id, ..FAKE_SOURCE }).contains("id "), "{id:?}");
    }
    assert!(problems_of(ConnectorDescriptor { version: 0, ..FAKE_SOURCE }).contains("version"));
    assert!(problems_of(ConnectorDescriptor { label: " ", ..FAKE_SOURCE }).contains("label"));
    assert!(problems_of(ConnectorDescriptor { description: "", ..FAKE_SOURCE }).contains("description"));
    assert!(valid_connector_id("telegram") && valid_connector_id("a-1-b") && valid_connector_id("x"));
}

#[test]
fn invalid_fields_are_reported() {
    static BAD_KEY: [ConfigField; 2] = [
        ConfigField { key: "Bad-Key", label: "x", help: "", required: false, kind: FieldKind::Bool { default: false } },
        ConfigField { key: "Bad-Key", label: "x", help: "", required: false, kind: FieldKind::Bool { default: false } },
    ];
    let p = problems_of(ConnectorDescriptor { config_fields: &BAD_KEY, ..FAKE_SOURCE });
    assert!(p.contains("non valida") && p.contains("duplicata"), "{p}");

    static BAD_KINDS: [ConfigField; 5] = [
        ConfigField {
            key: "t",
            label: "x",
            help: "",
            required: false,
            kind: FieldKind::Text { max_len: 0, placeholder: "" },
        },
        ConfigField { key: "n", label: "x", help: "", required: false, kind: FieldKind::Number { min: 5, max: 1 } },
        ConfigField { key: "s", label: "x", help: "", required: false, kind: FieldKind::Select { options: &[] } },
        ConfigField { key: "d", label: " ", help: "", required: false, kind: FieldKind::Workspace },
        ConfigField {
            key: "o",
            label: "x",
            help: "",
            required: false,
            kind: FieldKind::Select {
                options: &[SelectOption { value: "a", label: "A" }, SelectOption { value: "a", label: "B" }],
            },
        },
    ];
    let p = problems_of(ConnectorDescriptor { config_fields: &BAD_KINDS, ..FAKE_SOURCE });
    for expected in ["max_len", "min > max", "senza opzioni", "label vuota", "opzione vuota o duplicata"] {
        assert!(p.contains(expected), "{expected}: {p}");
    }
}

#[test]
fn secrets_are_allowed_in_config_but_not_in_filters_and_contacts_have_no_filters() {
    static SECRET_FILTER: [ConfigField; 1] =
        [ConfigField { key: "k", label: "x", help: "", required: false, kind: FieldKind::Secret }];
    let p = problems_of(ConnectorDescriptor { filter_fields: &SECRET_FILTER, ..FAKE_SOURCE });
    assert!(p.contains("filtro"), "{p}");
    let p = problems_of(ConnectorDescriptor { filter_fields: &SECRET_FILTER, ..FAKE_CONTACT });
    assert!(p.contains("contact non ha filter_fields"), "{p}");
}

#[test]
fn permissions_must_match_the_kind() {
    use HubCapability::*;
    static SOURCE_BAD: [HubCapability; 3] = [DeliverMessages, ReceiveNotices, SendAsUser];
    static CONTACT_BAD: [HubCapability; 3] = [ReceiveNotices, DeliverMessages, WakeSecretaries];
    let perms = |hub| Permissions { hosts: &["api.example.com"], hub };
    let p = problems_of(ConnectorDescriptor { permissions: perms(&SOURCE_BAD), ..FAKE_SOURCE });
    assert!(p.contains("receive_notices") && p.contains("send_as_user") && !p.contains("deliver_messages"), "{p}");
    let p = problems_of(ConnectorDescriptor { permissions: perms(&CONTACT_BAD), ..FAKE_CONTACT });
    assert!(p.contains("deliver_messages") && p.contains("wake_secretaries") && !p.contains("receive_notices"), "{p}");
    static DUP: [HubCapability; 2] = [DeliverMessages, DeliverMessages];
    assert!(problems_of(ConnectorDescriptor { permissions: perms(&DUP), ..FAKE_SOURCE }).contains("duplicata"));
}

#[test]
fn hosts_must_be_lowercase_plain_hostnames() {
    for host in ["api.telegram.org", "a.b", "x-1.example.co.uk", "xn--bcher-kva.example"] {
        assert!(valid_host(host), "{host}");
    }
    for host in [
        "",
        "localhost",
        "Api.Telegram.org",
        "https://api.telegram.org",
        "api.telegram.org:443",
        "api.telegram.org/x",
        "*.telegram.org",
        "-a.com",
        "a-.com",
        "a..com",
        ".a.com",
        "a.com.",
        "127.0.0.1",
        "10.0.0.1",
        "a b.com",
        "bücher.de",
        "user@a.com",
    ] {
        assert!(!valid_host(host), "{host:?}");
    }
    assert!(!valid_host(&format!("{}.com", "a".repeat(64))));
    static BAD_HOSTS: [&str; 2] = ["Api.Example.com", "api.example.com"];
    static DUP_HOSTS: [&str; 2] = ["api.example.com", "api.example.com"];
    let with_hosts = |hosts| Permissions { hosts, hub: FAKE_SOURCE.permissions.hub };
    assert!(problems_of(ConnectorDescriptor { permissions: with_hosts(&BAD_HOSTS), ..FAKE_SOURCE }).contains("host"));
    assert!(
        problems_of(ConnectorDescriptor { permissions: with_hosts(&DUP_HOSTS), ..FAKE_SOURCE }).contains("duplicato")
    );
}

#[test]
fn actions_and_slots_are_checked() {
    static PAIR_ON_SOURCE: [Action; 1] = [Action { id: ActionId::Pair, label: "Abbina", help: "" }];
    assert!(problems_of(ConnectorDescriptor { actions: &PAIR_ON_SOURCE, ..FAKE_SOURCE })
        .contains("solo dei connettori contact"));
    static DUP_ACTIONS: [Action; 2] = [
        Action { id: ActionId::Test, label: "Prova", help: "" },
        Action { id: ActionId::Test, label: "Prova", help: "" },
    ];
    assert!(problems_of(ConnectorDescriptor { actions: &DUP_ACTIONS, ..FAKE_SOURCE }).contains("duplicata"));
    static BAD_SLOTS: [&str; 2] = ["Bad Slot", "api_token"];
    let p = problems_of(ConnectorDescriptor { internal_secret_slots: &BAD_SLOTS, ..FAKE_SOURCE });
    assert!(p.contains("non valido") && p.contains("uguale a un campo secret"), "{p}");
}

#[test]
fn descriptor_limits_cannot_loosen_the_runtime_ones() {
    let limits = |items_per_round, min_interval_secs| DescriptorLimits {
        items_per_round,
        min_interval_secs,
        default_interval_secs: None,
    };
    assert!(
        problems_of(ConnectorDescriptor { limits: limits(Some(21), None), ..FAKE_SOURCE }).contains("items_per_round")
    );
    assert!(
        problems_of(ConnectorDescriptor { limits: limits(Some(0), None), ..FAKE_SOURCE }).contains("items_per_round")
    );
    assert!(problems_of(ConnectorDescriptor { limits: limits(None, Some(30)), ..FAKE_SOURCE }).contains("min_interval"));
    assert!(validate_descriptor(&ConnectorDescriptor { limits: limits(Some(5), Some(120)), ..FAKE_SOURCE }).is_empty());
}

#[test]
fn the_default_interval_respects_the_floors() {
    let limits = |min_interval_secs, default_interval_secs| DescriptorLimits {
        items_per_round: None,
        min_interval_secs,
        default_interval_secs,
    };
    let with = |l| problems_of(ConnectorDescriptor { limits: l, ..FAKE_SOURCE });
    assert!(with(limits(None, Some(30))).contains("default_interval_secs"));
    assert!(with(limits(Some(300), Some(120))).contains("default_interval_secs"));
    assert!(
        validate_descriptor(&ConnectorDescriptor { limits: limits(Some(300), Some(300)), ..FAKE_SOURCE }).is_empty()
    );
    assert!(validate_descriptor(&ConnectorDescriptor { limits: limits(None, Some(120)), ..FAKE_SOURCE }).is_empty());
}

#[test]
fn slots_and_capabilities_are_read_from_the_descriptor() {
    assert_eq!(FAKE_SOURCE.secret_slots(), vec!["api_token", "refresh_token"]);
    assert!(FAKE_SOURCE.declares_slot("refresh_token") && !FAKE_SOURCE.declares_slot("other"));
    assert!(
        FAKE_SOURCE.has_capability(HubCapability::DeliverMessages)
            && !FAKE_SOURCE.has_capability(HubCapability::SendAsUser)
    );
    assert!(FAKE_SOURCE.has_action(ActionId::Test) && !FAKE_SOURCE.has_action(ActionId::Pair));
    assert_eq!(ActionId::parse("pair"), Some(ActionId::Pair));
    assert_eq!(ActionId::parse("Pair"), None);
}

// ---- valori di configurazione e di filtro ----

fn fields() -> Vec<ConfigField> {
    vec![
        field("name", true, text(10)),
        field("note", false, text(20)),
        field("count", false, FieldKind::Number { min: 1, max: 10 }),
        field("flag", false, FieldKind::Bool { default: false }),
        field("mode", false, FieldKind::Select { options: OPTIONS }),
        field("ws", false, FieldKind::Workspace),
        field("target", false, FieldKind::Thread),
        field("token", false, FieldKind::Secret),
    ]
}

fn check(value: serde_json::Value) -> Result<(), String> {
    validate_values(&fields(), &value)
}

#[test]
fn valid_values_pass() {
    assert!(check(json!({"name": "mario"})).is_ok());
    let full = json!({
        "name": " mario ", "note": "", "count": 10, "flag": true, "mode": "b", "ws": "ws-1700000000000-1",
        "target": {"channel": "general", "thread_id": "thr-1-1"}
    });
    assert!(check(full).is_ok());
    assert!(check(json!({"name": "ok", "count": null})).is_ok(), "null = assente per un campo facoltativo");
}

#[test]
fn only_objects_are_accepted() {
    for bad in [json!(null), json!([]), json!("x"), json!(3)] {
        assert!(check(bad).is_err());
    }
}

#[test]
fn required_fields_must_be_present_and_non_empty() {
    assert!(check(json!({})).unwrap_err().contains("«name»"));
    assert!(check(json!({"name": null})).is_err());
    assert!(check(json!({"name": "   "})).is_err());
    assert!(validate_values(&[field("flag", true, FieldKind::Bool { default: false })], &json!({})).is_err());
}

#[test]
fn unknown_keys_are_rejected_without_echoing_long_text() {
    let err = check(json!({"name": "a", "extra": 1})).unwrap_err();
    assert!(err.contains("sconosciuto") && err.contains("extra"), "{err}");
    let long = "x".repeat(500);
    let err = check(json!({"name": "a", long.clone(): 1})).unwrap_err();
    assert!(err.len() < 200, "{err}");
}

#[test]
fn a_secret_field_must_not_appear_in_the_config() {
    let err = check(json!({"name": "a", "token": "sk-super-segreto"})).unwrap_err();
    assert!(err.contains("segreto") && err.contains("token"), "{err}");
    assert!(!err.contains("sk-super-segreto"));
    // Anche vuoto o null: la chiave non deve esserci.
    assert!(check(json!({"name": "a", "token": null})).is_err());
    assert!(check(json!({"name": "a", "token": ""})).is_err());
}

#[test]
fn types_and_limits_are_enforced() {
    let bad = [
        json!({"name": 5}),
        json!({"name": "x".repeat(11)}),
        json!({"name": "a\nb"}),
        json!({"name": "a", "note": "x".repeat(21)}),
        json!({"name": "a", "count": 0}),
        json!({"name": "a", "count": 11}),
        json!({"name": "a", "count": 1.5}),
        json!({"name": "a", "count": "3"}),
        json!({"name": "a", "flag": "yes"}),
        json!({"name": "a", "mode": "c"}),
        json!({"name": "a", "mode": 1}),
        json!({"name": "a", "ws": "general"}),
        json!({"name": "a", "ws": "../x"}),
        json!({"name": "a", "target": "thr-1"}),
        json!({"name": "a", "target": {"channel": "general"}}),
        json!({"name": "a", "target": {"channel": "general", "thread_id": "thr 1"}}),
        json!({"name": "a", "target": {"channel": "../x", "thread_id": "thr-1"}}),
        json!({"name": "a", "target": {"channel": "general", "thread_id": "t", "extra": 1}}),
    ];
    for value in bad {
        assert!(check(value.clone()).is_err(), "{value}");
    }
    // Il limite si misura in caratteri, dopo il trim.
    assert!(check(json!({"name": format!("  {}  ", "è".repeat(10))})).is_ok());
}

#[test]
fn errors_never_echo_the_rejected_value() {
    let err = check(json!({"name": "x".repeat(11), "mode": "SEGRETO"})).unwrap_err();
    assert!(!err.contains("SEGRETO") && !err.contains("xxxxxxxxxxx"));
    let err = check(json!({"name": "a", "mode": "SEGRETO"})).unwrap_err();
    assert!(!err.contains("SEGRETO"));
}

#[test]
fn oversized_values_are_rejected() {
    let huge = json!({"name": "a", "note": "x".repeat(crate::connectors::limits::CONFIG_JSON_MAX)});
    assert!(check(huge).unwrap_err().contains("troppo grande"));
}

#[test]
fn text_fields_are_trimmed_by_normalize() {
    let normalized = normalize_values(&fields(), json!({"name": "  mario\n", "mode": " b ", "count": 3}));
    assert_eq!(normalized, json!({"name": "mario", "mode": " b ", "count": 3}));
    assert_eq!(normalize_values(&fields(), json!([1])), json!([1]));
}

#[test]
fn keys_follow_the_slot_format() {
    assert!(valid_key("api_token") && valid_key("a1"));
    for bad in ["", "A", "a-b", "a b", "à", &"a".repeat(41)] {
        assert!(!valid_key(bad), "{bad:?}");
    }
}

#[test]
fn the_descriptor_serializes_for_the_frontend() {
    let v = serde_json::to_value(FAKE_SOURCE).unwrap();
    assert_eq!(v["kind"], "source");
    assert_eq!(v["permissions"]["hub"][0], "deliver_messages");
    assert_eq!(v["config_fields"][0]["kind"]["kind"], "secret");
}
