use super::*;
use crate::connectors::contracts::{
    ActionOutcome, ActiveRoute, BoxFuture, ConnectorContext, ConnectorError, PollBatch,
};
use crate::connectors::testing::{new_contact, new_source, NO_WAKE_SOURCE};
use serde_json::json;

fn input(filter: Value, wake: &str) -> RouteInput {
    RouteInput {
        label: "  Posta  ".to_string(),
        filter,
        channel: "general".to_string(),
        thread: RouteThread::NewPerItem,
        wake: wake.to_string(),
        enabled: true,
    }
}

fn route_of(draft: RouteDraft) -> ConnectorRoute {
    new_route("rte-1-1".to_string(), "con-1-1", draft, 5)
}

#[test]
fn ids_labels_and_slots_are_checked() {
    assert!(check_instance_id("con-1700000000000-1").is_ok());
    assert!(check_instance_id("../x").is_err() && check_instance_id("rte-1-1").is_err());
    assert!(check_route_id("rte-1-1").is_ok() && check_route_id("con-1-1").is_err());
    assert!(check_link_id("lnk-1-1").is_ok() && check_link_id("").is_err());
    assert_eq!(check_label("  Casella  ").unwrap(), "Casella");
    for bad in ["", "   ", "a\nb", &"x".repeat(61)] {
        assert!(check_label(bad).is_err(), "{bad:?}");
    }
    let source = new_source();
    let descriptor = source.descriptor();
    assert!(check_slot(descriptor, "api_token").is_ok() && check_slot(descriptor, "refresh_token").is_ok());
    assert!(check_slot(descriptor, "altro").is_err());
}

#[test]
fn actions_must_be_known_and_declared() {
    let (source, contact) = (new_source(), new_contact());
    assert_eq!(check_action(source.descriptor(), "test").unwrap(), ActionId::Test);
    assert!(check_action(source.descriptor(), "pair").is_err(), "dichiarata solo dal contatto");
    assert!(check_action(contact.descriptor(), "pair").is_ok());
    assert!(check_action(contact.descriptor(), "boom").is_err());
}

#[test]
fn config_is_trimmed_validated_and_never_carries_secrets() {
    let source = new_source();
    let ok = check_config(source, json!({"mailbox": "  posta  ", "interval_secs": 120})).unwrap();
    assert_eq!(ok, json!({"mailbox": "posta", "interval_secs": 120}));
    let secret = check_config(source, json!({"api_token": "tok-SEGRETO"})).unwrap_err();
    assert!(secret.contains("segreto") && !secret.contains("SEGRETO"), "{secret}");
    assert!(check_config(source, json!({"sconosciuto": 1})).is_err());
    assert!(check_config(source, json!({"interval_secs": 5})).is_err(), "sotto il minimo del campo");
    assert!(check_config(source, json!("testo")).is_err());
    let semantic = check_config(source, json!({"mailbox": "vietata"})).unwrap_err();
    assert!(semantic.contains("casella non ammessa"), "{semantic}");
}

#[test]
fn a_route_is_validated_generically_and_by_the_connector() {
    let source = new_source();
    let draft = check_route(source, input(json!({"query": "  da:capo "}), "none")).unwrap();
    assert_eq!(draft.label, "Posta");
    assert_eq!(draft.filter, json!({"query": "da:capo"}));
    assert_eq!(draft.channel, Channel::General);
    assert!(check_route(source, input(json!({}), "none")).is_err(), "campo obbligatorio");
    assert!(check_route(source, input(json!({"query": "x", "altro": 1}), "none")).is_err());
    let broad = check_route(source, input(json!({"query": "*"}), "none")).unwrap_err();
    assert!(broad.contains("ricerca troppo ampia"), "{broad}");
}

#[test]
fn channel_thread_and_wake_are_validated() {
    let source = new_source();
    let f = || json!({"query": "x"});
    let mut bad_channel = input(f(), "none");
    bad_channel.channel = "../x".to_string();
    assert!(check_route(source, bad_channel).is_err());
    let mut bad_thread = input(f(), "none");
    bad_thread.thread = RouteThread::Existing { thread_id: "no spazi".to_string() };
    assert!(check_route(source, bad_thread).is_err());
    assert!(check_route(source, input(f(), "tutti")).is_err());
    assert!(check_route(source, input(f(), "secretary:")).is_err());
    let draft = check_route(source, input(f(), "secretary:Luna")).unwrap();
    assert_eq!(draft.wake, RouteWake::Secretary("Luna".to_string()));
    assert!(check_route(source, input(f(), "owner")).is_ok());
}

/// Una fonte il cui descrittore non dichiara `wake_secretaries`.
struct NoWake;

impl Connector for NoWake {
    fn descriptor(&self) -> &'static ConnectorDescriptor {
        &NO_WAKE_SOURCE
    }

    fn migrate_config(&self, _: u32, config: Value) -> Result<Value, String> {
        Ok(config)
    }

    fn validate_config(&self, _: &Value) -> Result<(), String> {
        Ok(())
    }

    fn action<'a>(
        &'a self,
        _: &'a ConnectorContext,
        _: ActionId,
    ) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>> {
        Box::pin(async { Err(ConnectorError::Fatal("non usata".to_string())) })
    }
}

impl SourceConnector for NoWake {
    fn validate_filter(&self, _: &Value) -> Result<(), String> {
        Ok(())
    }

    fn poll<'a>(
        &'a self,
        _: &'a ConnectorContext,
        _: &'a [ActiveRoute],
        _: Option<Value>,
    ) -> BoxFuture<'a, Result<PollBatch, ConnectorError>> {
        Box::pin(async { Err(ConnectorError::Fatal("non usata".to_string())) })
    }
}

#[test]
fn waking_requires_the_capability() {
    let f = || json!({"query": "x"});
    assert!(check_route(&NoWake, input(f(), "none")).is_ok());
    let err = check_route(&NoWake, input(f(), "owner")).unwrap_err();
    assert!(err.contains("svegliare"), "{err}");
    assert!(check_route(&NoWake, input(f(), "secretary:Luna")).is_err());
}

#[test]
fn only_sources_have_routes() {
    assert!(source_of(ConnectorImpl::Source(new_source())).is_ok());
    assert!(source_of(ConnectorImpl::Contact(new_contact())).is_err());
}

#[test]
fn the_thread_is_looked_up_only_when_it_matters() {
    let source = new_source();
    let mut existing = input(json!({"query": "x"}), "none");
    existing.thread = RouteThread::Existing { thread_id: "thr-1".to_string() };
    let draft = check_route(source, existing).unwrap();
    assert_eq!(thread_to_verify(None, &draft), Some("thr-1"), "regola nuova");
    let old = route_of(draft.clone());
    assert_eq!(thread_to_verify(Some(&old), &draft), None, "nulla di cambiato");
    let mut other_thread = draft.clone();
    other_thread.thread = RouteThread::Existing { thread_id: "thr-2".to_string() };
    assert_eq!(thread_to_verify(Some(&old), &other_thread), Some("thr-2"));
    let mut other_channel = draft.clone();
    other_channel.channel = Channel::Workspace("ws-1".to_string());
    assert_eq!(thread_to_verify(Some(&old), &other_channel), Some("thr-1"));
    let mut disabled = old.clone();
    disabled.enabled = false;
    assert_eq!(thread_to_verify(Some(&disabled), &draft), Some("thr-1"), "riabilitata");
    let mut still_off = draft.clone();
    still_off.enabled = false;
    assert_eq!(thread_to_verify(Some(&disabled), &still_off), None);
    let mut per_item = draft;
    per_item.thread = RouteThread::NewPerItem;
    assert_eq!(thread_to_verify(None, &per_item), None);
}

#[test]
fn applying_a_draft_clears_the_auto_disable_error_when_it_no_longer_applies() {
    let source = new_source();
    let draft = check_route(source, input(json!({"query": "x"}), "none")).unwrap();
    let mut route = route_of(draft.clone());
    route.enabled = false;
    route.error = Some("thread non più disponibile".to_string());
    let mut renamed = draft.clone();
    renamed.label = "Nuova".to_string();
    renamed.enabled = false;
    apply_draft(&mut route, renamed, 9);
    assert_eq!((route.label.as_str(), route.updated_at), ("Nuova", 9));
    assert!(route.error.is_some(), "ancora spenta e stesso bersaglio");
    let mut moved = draft.clone();
    moved.enabled = false;
    moved.channel = Channel::Workspace("ws-1".to_string());
    apply_draft(&mut route, moved, 10);
    assert!(route.error.is_none(), "bersaglio cambiato");
    route.error = Some("x".to_string());
    apply_draft(&mut route, draft, 11);
    assert!(route.enabled && route.error.is_none());
    assert_eq!(route.created_at, 5);
}

#[test]
fn routes_are_filtered_by_channel_and_thread() {
    let source = new_source();
    let make = |id: &str, channel: &str, thread: RouteThread| {
        let mut i = input(json!({"query": "x"}), "none");
        i.channel = channel.to_string();
        i.thread = thread;
        let mut r = route_of(check_route(source, i).unwrap());
        r.id = id.to_string();
        r
    };
    let routes = vec![
        make("rte-1-1", "general", RouteThread::Existing { thread_id: "thr-1".to_string() }),
        make("rte-1-2", "ws-1", RouteThread::Existing { thread_id: "thr-1".to_string() }),
        make("rte-1-3", "general", RouteThread::NewPerItem),
    ];
    let ids = |f: &RouteFilter| filter_routes(&routes, f).into_iter().map(|r| r.id).collect::<Vec<_>>();
    assert_eq!(ids(&RouteFilter::default()).len(), 3);
    let general = parse_route_filter(Some("general"), None).unwrap();
    assert_eq!(ids(&general), ["rte-1-1", "rte-1-3"]);
    let thread = parse_route_filter(None, Some("thr-1")).unwrap();
    assert_eq!(ids(&thread), ["rte-1-1", "rte-1-2"]);
    let both = parse_route_filter(Some("ws-1"), Some("thr-1")).unwrap();
    assert_eq!(ids(&both), ["rte-1-2"]);
    assert!(parse_route_filter(Some("../x"), None).is_err());
    assert!(parse_route_filter(None, Some("con spazi")).is_err());
}
