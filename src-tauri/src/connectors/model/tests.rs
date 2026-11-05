use super::*;
use serde_json::json;

fn instance(id: &str) -> ConnectorInstance {
    ConnectorInstance {
        id: id.to_string(),
        connector: "fake-source".to_string(),
        label: "Etichetta".to_string(),
        enabled: false,
        config: json!({}),
        config_version: 1,
        notices: ConnectorNotices::default(),
        created_at: 1,
        updated_at: 1,
    }
}

fn route(id: &str, instance: &str, channel: Channel, thread: RouteThread) -> ConnectorRoute {
    ConnectorRoute {
        id: id.to_string(),
        instance: instance.to_string(),
        label: "Regola".to_string(),
        filter: json!({}),
        channel,
        thread,
        wake: RouteWake::None,
        enabled: true,
        error: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn link(id: &str, instance: &str, current: Option<LinkThread>) -> ConnectorLink {
    ConnectorLink {
        id: id.to_string(),
        instance: instance.to_string(),
        external_account: "42".to_string(),
        external_chat: "42".to_string(),
        display_name: "Mario".to_string(),
        current,
        channel: None,
        muted_until: None,
        created_at: 1,
    }
}

fn ws(id: &str) -> Channel {
    Channel::parse(id).unwrap()
}

fn existing() -> RouteThread {
    RouteThread::Existing { thread_id: "thr-1-1".to_string() }
}

#[test]
fn the_wake_is_a_string_on_the_wire() {
    for (wake, text) in [
        (RouteWake::None, "none"),
        (RouteWake::Owner, "owner"),
        (RouteWake::Secretary("mario".to_string()), "secretary:mario"),
    ] {
        assert_eq!(serde_json::to_value(&wake).unwrap(), json!(text));
        assert_eq!(serde_json::from_value::<RouteWake>(json!(text)).unwrap(), wake);
    }
    assert!(RouteWake::Owner.wakes() && !RouteWake::None.wakes());
}

#[test]
fn invalid_wakes_are_rejected() {
    let too_long = format!("secretary:{}", "x".repeat(BOT_NAME_MAX + 1));
    for bad in ["", "None", "owner ", "secretary:", "secretary: mario", "secretary:ma\nrio", "secretary", &too_long] {
        assert!(RouteWake::parse(bad).is_err(), "{bad:?}");
        assert!(serde_json::from_value::<RouteWake>(json!(bad)).is_err());
    }
    assert!(serde_json::from_value::<RouteWake>(json!(5)).is_err());
}

#[test]
fn the_wake_maps_to_the_hub_wake() {
    assert_eq!(RouteWake::None.to_external(), ExternalWake::None);
    assert_eq!(RouteWake::Owner.to_external(), ExternalWake::Owner);
    assert_eq!(RouteWake::Secretary("m".into()).to_external(), ExternalWake::Secretary("m".into()));
}

#[test]
fn the_thread_mode_is_tagged() {
    assert_eq!(serde_json::to_value(existing()).unwrap(), json!({"mode": "existing", "thread_id": "thr-1-1"}));
    assert_eq!(serde_json::to_value(RouteThread::NewPerItem).unwrap(), json!({"mode": "new_per_item"}));
    assert_eq!(serde_json::to_value(RouteThread::PerConversation).unwrap(), json!({"mode": "per_conversation"}));
    assert!(serde_json::from_value::<RouteThread>(json!({"mode": "other"})).is_err());
}

#[test]
fn notices_default_to_on_and_fill_missing_fields() {
    assert!(ConnectorNotices::default().all);
    let partial: ConnectorNotices = serde_json::from_value(json!({"replies": false})).unwrap();
    assert!(!partial.replies && partial.all && partial.questions && partial.sources);
    let instance: ConnectorInstance = serde_json::from_value(json!({
        "id": "con-1-1", "connector": "x", "label": "L", "enabled": false, "config": {},
        "config_version": 1, "created_at": 1, "updated_at": 1
    }))
    .unwrap();
    assert_eq!(instance.notices, ConnectorNotices::default());
}

#[test]
fn the_instance_dto_has_no_secret_fields() {
    let value = serde_json::to_value(instance("con-1-1")).unwrap();
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["config", "config_version", "connector", "created_at", "enabled", "id", "label", "notices", "updated_at"]
    );
}

#[test]
fn structural_checks_accept_unknown_connectors_and_reject_bad_shapes() {
    let mut unknown = instance("con-1-1");
    unknown.connector = "connettore-futuro".to_string();
    assert!(unknown.check().is_ok());
    for mutate in [
        (|i: &mut ConnectorInstance| i.id = "x".to_string()) as fn(&mut ConnectorInstance),
        |i| i.connector = "Bad_ID".to_string(),
        |i| i.label = " ".to_string(),
        |i| i.label = "x".repeat(LABEL_MAX + 1),
        |i| i.label = "a\nb".to_string(),
        |i| i.config = json!([1]),
    ] {
        let mut bad = instance("con-1-1");
        mutate(&mut bad);
        assert!(bad.check().is_err());
    }
    assert!(route("rte-1-1", "con-1-1", Channel::General, existing()).check().is_ok());
    assert!(route("con-1-1", "con-1-1", Channel::General, existing()).check().is_err());
    let bad_thread = RouteThread::Existing { thread_id: "thr 1".to_string() };
    assert!(route("rte-1-1", "con-1-1", Channel::General, bad_thread).check().is_err());
    let mut bad = link("lnk-1-1", "con-1-1", None);
    assert!(bad.check().is_ok());
    bad.external_account = "x".repeat(EXTERNAL_REF_MAX + 1);
    assert!(bad.check().is_err());
}

#[test]
fn the_limits_are_checked_per_kind_of_entity() {
    let mut file = ConnectorsFile::default();
    for i in 0..MAX_INSTANCES {
        file.instances.push(instance(&format!("con-1-{i}")));
    }
    assert!(file.check_instance_capacity().is_err());
    file.instances.pop();
    assert!(file.check_instance_capacity().is_ok());

    for i in 0..MAX_ROUTES_PER_INSTANCE {
        file.routes.push(route(&format!("rte-1-{i}"), "con-1-0", Channel::General, existing()));
    }
    assert!(file.check_route_capacity("con-1-0").is_err());
    assert!(file.check_route_capacity("con-1-1").is_ok(), "il tetto e' per istanza");

    for i in 0..MAX_LINKS_PER_INSTANCE {
        file.links.push(link(&format!("lnk-1-{i}"), "con-1-0", None));
    }
    assert!(file.check_link_capacity("con-1-0").is_err());
    assert!(file.check_link_capacity("con-1-1").is_ok());
}

#[test]
fn removing_an_instance_removes_its_routes_and_links_only() {
    let mut file = ConnectorsFile {
        instances: vec![instance("con-1-1"), instance("con-1-2")],
        routes: vec![
            route("rte-1-1", "con-1-1", Channel::General, existing()),
            route("rte-1-2", "con-1-2", Channel::General, existing()),
        ],
        links: vec![link("lnk-1-1", "con-1-1", None), link("lnk-1-2", "con-1-2", None)],
    };
    assert!(file.remove_instance("con-1-1"));
    assert!(!file.remove_instance("con-1-1"));
    assert_eq!(file.instances.len(), 1);
    assert_eq!((file.routes.len(), file.links.len()), (1, 1));
    assert_eq!((file.routes[0].id.as_str(), file.links[0].id.as_str()), ("rte-1-2", "lnk-1-2"));
}

#[test]
fn forgetting_a_channel_drops_its_routes_and_resets_the_links_pointing_there() {
    let here = LinkThread { channel: ws("ws-1-1"), thread_id: "thr-1-1".to_string() };
    let elsewhere = LinkThread { channel: Channel::General, thread_id: "thr-1-2".to_string() };
    let mut file = ConnectorsFile {
        instances: vec![instance("con-1-1")],
        routes: vec![
            route("rte-1-1", "con-1-1", ws("ws-1-1"), existing()),
            route("rte-1-2", "con-1-1", Channel::General, existing()),
            route("rte-1-3", "con-1-1", ws("ws-1-1"), RouteThread::PerConversation),
        ],
        links: vec![link("lnk-1-1", "con-1-1", Some(here)), link("lnk-1-2", "con-1-1", Some(elsewhere.clone()))],
    };
    let forgotten = file.forget_channel(&ws("ws-1-1"));
    assert_eq!(
        forgotten.routes,
        vec![("con-1-1".to_string(), "rte-1-1".to_string()), ("con-1-1".to_string(), "rte-1-3".to_string())]
    );
    assert_eq!(forgotten.links_reset, 1);
    assert_eq!(file.routes.len(), 1);
    assert_eq!((file.links[0].current.clone(), file.links[1].current.clone()), (None, Some(elsewhere)));
    assert_eq!(file.forget_channel(&ws("ws-1-1")), ForgottenChannel::default());
}

#[test]
fn forgetting_a_channel_also_clears_the_channel_chosen_with_ws() {
    let mut chosen = link("lnk-1-1", "con-1-1", None);
    chosen.channel = Some(ws("ws-1-1"));
    let mut other = link("lnk-1-2", "con-1-1", None);
    other.channel = Some(Channel::General);
    let mut file = ConnectorsFile { links: vec![chosen, other], ..ConnectorsFile::default() };
    let forgotten = file.forget_channel(&ws("ws-1-1"));
    assert_eq!(forgotten.links_reset, 1);
    assert_eq!((file.links[0].channel.clone(), file.links[1].channel.clone()), (None, Some(Channel::General)));
}

#[test]
fn a_link_saved_before_the_channel_field_still_loads_and_an_unset_channel_is_not_written() {
    let old = r#"{"id":"lnk-1-1","instance":"con-1-1","external_account":"42","external_chat":"42","display_name":"M","created_at":1}"#;
    let parsed: ConnectorLink = serde_json::from_str(old).unwrap();
    assert_eq!(parsed.channel, None);
    assert!(serde_json::to_value(&parsed).unwrap().get("channel").is_none());
}

#[test]
fn lookups_find_entities_by_id_and_instance() {
    let mut file = ConnectorsFile {
        instances: vec![instance("con-1-1")],
        routes: vec![route("rte-1-1", "con-1-1", Channel::General, existing())],
        links: vec![link("lnk-1-1", "con-1-1", None)],
    };
    assert!(file.instance("con-1-1").is_some() && file.instance("con-9-9").is_none());
    file.instance_mut("con-1-1").unwrap().enabled = true;
    assert!(file.instance("con-1-1").unwrap().enabled);
    assert_eq!((file.routes_of("con-1-1").count(), file.links_of("con-1-1").count()), (1, 1));
    assert_eq!((file.routes_of("con-1-2").count(), file.links_of("con-1-2").count()), (0, 0));
}
