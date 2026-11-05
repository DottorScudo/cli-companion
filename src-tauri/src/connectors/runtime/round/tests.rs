use super::*;
use crate::connectors::contracts::PollBatch;
use crate::connectors::descriptor::ConnectorDescriptor;
use crate::connectors::model::RouteWake;
use crate::connectors::state::MAIN_CURSOR;
use crate::connectors::testing::{
    batch, item, new_source, route, test_context, FakeSource, HubCall, TestContext, FAKE_SOURCE, NO_WAKE_SOURCE,
};
use crate::hub::Channel;
use serde_json::{json, Value};

struct Rig {
    t: TestContext,
    source: &'static FakeSource,
    routes: Vec<ConnectorRoute>,
    max_items: usize,
}

impl Rig {
    fn new(routes: Vec<ConnectorRoute>) -> Self {
        Self::with_descriptor(&FAKE_SOURCE, routes)
    }

    fn with_descriptor(descriptor: &'static ConnectorDescriptor, routes: Vec<ConnectorRoute>) -> Self {
        Self { t: test_context(descriptor, json!({})), source: new_source(), routes, max_items: 20 }
    }

    fn round(&self) -> RoundResult {
        let rc = RoundCtx {
            source: self.source,
            ctx: &self.t.ctx,
            hub: &*self.t.hub,
            state: &self.t.state,
            routes: &self.routes,
            max_items: self.max_items,
        };
        tauri::async_runtime::block_on(run_round(&rc))
    }

    fn deliveries(&self) -> Vec<ExternalDelivery> {
        self.t
            .hub
            .calls()
            .into_iter()
            .filter_map(|c| match c {
                HubCall::Deliver(d) => Some(d),
                _ => None,
            })
            .collect()
    }

    fn cursor(&self) -> Option<Value> {
        self.t.state.lock().cursor(MAIN_CURSOR).cloned()
    }
}

fn new_per_item(id: &str) -> ConnectorRoute {
    route(id, RouteThread::NewPerItem, RouteWake::None)
}

fn three_items() -> PollBatch {
    batch(vec![item("rte-1-1", "a", None), item("rte-1-1", "b", None), item("rte-1-1", "c", None)], json!(7), false)
}

#[test]
fn a_batch_is_delivered_item_by_item_and_then_the_cursor_is_saved() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.source.push(Ok(three_items()));
    let result = rig.round();
    assert_eq!(result, RoundResult { end: RoundEnd::Done { more: false }, disable: Vec::new() });
    let deliveries = rig.deliveries();
    assert_eq!(deliveries.len(), 3);
    assert_eq!(deliveries[0].target, DeliveryTarget::New { title: "Titolo a".to_string() });
    assert_eq!(deliveries[1].origin.external_id, "b");
    assert!(
        matches!(&deliveries[2].author, Author::External { connector, name, .. } if connector == "fake-source" && name == "Mario")
    );
    assert_eq!(rig.cursor(), Some(json!(7)));
    assert!(rig.t.state.lock().is_seen("rte-1-1", "c"));
}

#[test]
fn a_repolled_batch_is_not_delivered_twice() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.source.push(Ok(three_items()));
    rig.round();
    rig.source.push(Ok(three_items()));
    let again = rig.round();
    assert_eq!(again.end, RoundEnd::Done { more: false });
    assert_eq!(rig.deliveries().len(), 3, "nessuna consegna in piu'");
}

#[test]
fn a_duplicate_inside_one_batch_is_delivered_once() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    let items = vec![item("rte-1-1", "a", None), item("rte-1-1", "a", None)];
    rig.source.push(Ok(batch(items, json!(1), false)));
    rig.round();
    assert_eq!(rig.deliveries().len(), 1);
}

#[test]
fn the_cursor_is_saved_only_after_delivery_and_a_failed_item_is_redelivered() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.source.push(Ok(three_items()));
    *rig.t.hub.fail_from.lock().unwrap() = Some(2);
    let failed = rig.round();
    assert!(matches!(failed.end, RoundEnd::Failed(ConnectorError::Transient(_))), "{failed:?}");
    assert_eq!(rig.cursor(), None, "il cursore vecchio resta");
    assert!(rig.t.state.lock().is_seen("rte-1-1", "a") && !rig.t.state.lock().is_seen("rte-1-1", "b"));
    // Il HUB torna: lo stesso lotto arriva di nuovo e si consegnano solo b e c.
    *rig.t.hub.fail_from.lock().unwrap() = None;
    rig.source.push(Ok(three_items()));
    assert_eq!(rig.round().end, RoundEnd::Done { more: false });
    let ids: Vec<String> = rig.deliveries().iter().map(|d| d.origin.external_id.clone()).collect();
    assert_eq!(ids, vec!["a", "b", "b", "c"], "a una volta sola, b ritentato");
    assert_eq!(rig.cursor(), Some(json!(7)));
}

#[test]
fn a_poll_error_is_returned_without_touching_the_state() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.source.push(Err(ConnectorError::Auth("x".to_string())));
    assert_eq!(rig.round().end, RoundEnd::Failed(ConnectorError::Auth("x".to_string())));
    assert!(rig.deliveries().is_empty() && rig.cursor().is_none());
}

#[test]
fn only_the_last_item_of_each_thread_wakes() {
    let existing = route("rte-1-1", RouteThread::Existing { thread_id: "thr-9".to_string() }, RouteWake::Owner);
    let conv = route("rte-1-2", RouteThread::PerConversation, RouteWake::Secretary("mario".to_string()));
    let rig = Rig::new(vec![existing, conv]);
    rig.t.hub.add_thread(&Channel::General, "thr-9");
    let items = vec![
        item("rte-1-1", "e1", None),
        item("rte-1-2", "a1", Some("A")),
        item("rte-1-1", "e2", None),
        item("rte-1-2", "b1", Some("B")),
        item("rte-1-2", "a2", Some("A")),
        item("rte-1-1", "e3", None),
    ];
    rig.source.push(Ok(batch(items, json!(1), false)));
    assert_eq!(rig.round().end, RoundEnd::Done { more: false });
    let wakes: Vec<(String, ExternalWake)> =
        rig.deliveries().into_iter().map(|d| (d.origin.external_id, d.wake)).collect();
    let mario = || ExternalWake::Secretary("mario".to_string());
    assert_eq!(
        wakes,
        vec![
            ("e1".to_string(), ExternalWake::None),
            ("a1".to_string(), ExternalWake::None),
            ("e2".to_string(), ExternalWake::None),
            ("b1".to_string(), mario()),
            ("a2".to_string(), mario()),
            ("e3".to_string(), ExternalWake::Owner),
        ]
    );
}

#[test]
fn every_new_per_item_thread_keeps_its_own_wake() {
    let rig = Rig::new(vec![route("rte-1-1", RouteThread::NewPerItem, RouteWake::Owner)]);
    rig.source.push(Ok(three_items()));
    rig.round();
    assert!(rig.deliveries().iter().all(|d| d.wake == ExternalWake::Owner));
}

#[test]
fn the_wake_is_ignored_without_the_wake_capability() {
    let rig = Rig::with_descriptor(&NO_WAKE_SOURCE, vec![route("rte-1-1", RouteThread::NewPerItem, RouteWake::Owner)]);
    rig.source.push(Ok(three_items()));
    rig.round();
    assert!(rig.deliveries().iter().all(|d| d.wake == ExternalWake::None));
}

#[test]
fn a_vanished_existing_thread_disables_the_route_and_loses_nothing() {
    let gone = route("rte-1-1", RouteThread::Existing { thread_id: "thr-sparito".to_string() }, RouteWake::None);
    let mut rig = Rig::new(vec![gone, new_per_item("rte-1-2")]);
    let items = vec![item("rte-1-1", "x", None), item("rte-1-2", "y", None)];
    rig.source.push(Ok(batch(items.clone(), json!(5), false)));
    let result = rig.round();
    assert_eq!(result, RoundResult { end: RoundEnd::Retry, disable: vec!["rte-1-1".to_string()] });
    assert!(rig.deliveries().is_empty() && rig.cursor().is_none(), "niente salvato, niente perso");
    // Il supervisore salva la disabilitazione e rifa' il giro: l'altra regola consegna.
    rig.routes[0].enabled = false;
    rig.source.push(Ok(batch(items, json!(5), false)));
    assert_eq!(rig.round().end, RoundEnd::Done { more: false });
    let ids: Vec<String> = rig.deliveries().iter().map(|d| d.origin.external_id.clone()).collect();
    assert_eq!(ids, vec!["y"]);
    assert_eq!(rig.cursor(), Some(json!(5)));
}

#[test]
fn a_conversation_reuses_its_thread_and_the_mapping_is_recorded() {
    let rig = Rig::new(vec![route("rte-1-1", RouteThread::PerConversation, RouteWake::None)]);
    rig.source.push(Ok(batch(vec![item("rte-1-1", "m1", Some("conv"))], json!(1), false)));
    rig.round();
    assert_eq!(rig.t.state.lock().conversations.get("rte-1-1:conv"), Some("thr-fake-1"));
    rig.source.push(Ok(batch(
        vec![item("rte-1-1", "m2", Some("conv")), item("rte-1-1", "m3", Some("altra"))],
        json!(2),
        false,
    )));
    rig.round();
    let deliveries = rig.deliveries();
    assert_eq!(deliveries[1].target, DeliveryTarget::Existing("thr-fake-1".to_string()));
    assert!(matches!(deliveries[2].target, DeliveryTarget::New { .. }), "un'altra conversazione, un altro thread");
    assert_eq!(rig.t.state.lock().conversations.get("rte-1-1:altra"), Some("thr-fake-3"));
}

#[test]
fn two_items_of_a_new_conversation_in_one_round_share_the_new_thread() {
    let rig = Rig::new(vec![route("rte-1-1", RouteThread::PerConversation, RouteWake::None)]);
    let items = vec![item("rte-1-1", "m1", Some("conv")), item("rte-1-1", "m2", Some("conv"))];
    rig.source.push(Ok(batch(items, json!(1), false)));
    rig.round();
    assert_eq!(rig.deliveries()[1].target, DeliveryTarget::Existing("thr-fake-1".to_string()));
}

#[test]
fn a_mapping_to_a_vanished_thread_is_discarded_and_a_new_thread_is_opened() {
    let rig = Rig::new(vec![route("rte-1-1", RouteThread::PerConversation, RouteWake::None)]);
    rig.t.state.lock().conversations.insert("rte-1-1:conv", "thr-sparito");
    rig.source.push(Ok(batch(vec![item("rte-1-1", "m1", Some("conv"))], json!(1), false)));
    assert_eq!(rig.round().end, RoundEnd::Done { more: false });
    assert!(matches!(rig.deliveries()[0].target, DeliveryTarget::New { .. }));
    assert_eq!(rig.t.state.lock().conversations.get("rte-1-1:conv"), Some("thr-fake-1"));
}

#[test]
fn a_conversation_item_without_a_key_opens_a_new_thread() {
    let rig = Rig::new(vec![route("rte-1-1", RouteThread::PerConversation, RouteWake::None)]);
    rig.source.push(Ok(batch(vec![item("rte-1-1", "m1", None)], json!(1), false)));
    rig.round();
    assert!(matches!(rig.deliveries()[0].target, DeliveryTarget::New { .. }));
    assert_eq!(rig.t.state.lock().conversations.len(), 0);
}

#[test]
fn at_most_max_items_per_round_and_the_rest_waits_with_the_old_cursor() {
    let mut rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.max_items = 2;
    rig.source.push(Ok(three_items()));
    assert_eq!(rig.round().end, RoundEnd::Done { more: true });
    assert_eq!(rig.deliveries().len(), 2);
    assert_eq!(rig.cursor(), None);
    rig.source.push(Ok(three_items()));
    assert_eq!(rig.round().end, RoundEnd::Done { more: false });
    assert_eq!(rig.deliveries().len(), 3);
    assert_eq!(rig.cursor(), Some(json!(7)));
}

#[test]
fn the_more_flag_of_the_batch_is_passed_on() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    rig.source.push(Ok(batch(vec![item("rte-1-1", "a", None)], json!(1), true)));
    assert_eq!(rig.round().end, RoundEnd::Done { more: true });
}

#[test]
fn items_of_disabled_or_unknown_routes_are_skipped_and_no_active_route_means_no_poll() {
    let mut off = new_per_item("rte-1-1");
    off.enabled = false;
    let rig = Rig::new(vec![off, new_per_item("rte-1-2")]);
    let items = vec![item("rte-1-1", "a", None), item("rte-9-9", "b", None), item("rte-1-2", "c", None)];
    rig.source.push(Ok(batch(items, json!(3), false)));
    rig.round();
    assert_eq!(rig.deliveries().len(), 1);
    let mut none = Rig::new(vec![new_per_item("rte-1-1")]);
    none.routes[0].enabled = false;
    assert_eq!(none.round().end, RoundEnd::Done { more: false });
    assert_eq!(none.source.polls(), 0);
}

#[test]
fn state_of_deleted_routes_is_pruned_but_the_main_cursor_stays() {
    let rig = Rig::new(vec![new_per_item("rte-1-1")]);
    {
        let mut state = rig.t.state.lock();
        state.set_cursor(MAIN_CURSOR, json!(1));
        state.conversations.insert("rte-7-7:c", "thr-1");
        state.conversations.insert("rte-1-1:c", "thr-2");
    }
    rig.round();
    let state = rig.t.state.lock();
    assert_eq!((state.conversations.get("rte-7-7:c"), state.conversations.get("rte-1-1:c")), (None, Some("thr-2")));
    assert!(state.cursor(MAIN_CURSOR).is_some());
}
