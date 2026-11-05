//! Il contesto fa rispettare i permessi: senza la capacita' dichiarata il metodo
//! risponde "permesso non dichiarato" e la porta HUB non viene toccata.

use super::*;
use crate::connectors::descriptor::ActionId;
use crate::connectors::testing::{
    external_author, test_context, HubCall, TestContext, FAKE_CONTACT, FAKE_SOURCE, INSTANCE,
};
use crate::hub::ingress::{DeliveryTarget, ExternalDelivery, ExternalWake};
use crate::hub::send::UserMessage;
use crate::hub::{Channel, MessageOrigin};
use serde_json::json;

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tauri::async_runtime::block_on(future)
}

fn origin(connector: &str, instance: &str) -> MessageOrigin {
    MessageOrigin { connector: connector.to_string(), instance: instance.to_string(), external_id: "m-1".to_string() }
}

fn delivery(wake: ExternalWake) -> ExternalDelivery {
    ExternalDelivery {
        channel: Channel::General,
        target: DeliveryTarget::New { title: "t".to_string() },
        author: external_author(),
        body: "ciao".to_string(),
        files: Vec::new(),
        origin: origin("fake-source", INSTANCE),
        wake,
    }
}

fn user_message(origin: Option<MessageOrigin>) -> UserMessage {
    UserMessage {
        channel: Channel::General,
        thread_id: "thr-1".to_string(),
        body: "ciao".to_string(),
        reply_to: None,
        attachment_ids: Vec::new(),
        origin,
    }
}

fn denied(error: ConnectorError, capability: &str) {
    match error {
        ConnectorError::Fatal(m) => assert_eq!(m, format!("permesso non dichiarato: {capability}")),
        other => panic!("atteso Fatal, trovato {other:?}"),
    }
}

#[test]
fn deliver_requires_the_capability_and_never_reaches_the_hub_without_it() {
    let t = test_context(&FAKE_CONTACT, json!({}));
    denied(block_on(t.ctx.deliver(delivery(ExternalWake::None))).unwrap_err(), "deliver_messages");
    assert!(t.hub.calls().is_empty());
}

#[test]
fn deliver_goes_to_the_hub_with_the_capability() {
    let t = test_context(&FAKE_SOURCE, json!({}));
    let outcome = block_on(t.ctx.deliver(delivery(ExternalWake::Owner))).unwrap();
    assert!(!outcome.duplicate);
    assert!(matches!(t.hub.calls().as_slice(), [HubCall::Deliver(d)] if d.wake == ExternalWake::Owner));
}

#[test]
fn deliver_refuses_a_foreign_origin() {
    let t = test_context(&FAKE_SOURCE, json!({}));
    for (connector, instance) in [("other", INSTANCE), ("fake-source", "con-1-2")] {
        let mut d = delivery(ExternalWake::None);
        d.origin = origin(connector, instance);
        assert!(matches!(block_on(t.ctx.deliver(d)), Err(ConnectorError::Fatal(_))));
    }
    assert!(t.hub.calls().is_empty());
}

#[test]
fn deliver_ignores_the_wake_without_the_wake_capability() {
    static NO_WAKE: crate::connectors::descriptor::ConnectorDescriptor =
        crate::connectors::descriptor::ConnectorDescriptor {
            permissions: crate::connectors::descriptor::Permissions {
                hosts: &["api.fake-source.example"],
                hub: &[crate::connectors::descriptor::HubCapability::DeliverMessages],
            },
            ..FAKE_SOURCE
        };
    let t = test_context(&NO_WAKE, json!({}));
    block_on(t.ctx.deliver(delivery(ExternalWake::Secretary("mario".to_string())))).unwrap();
    assert!(matches!(t.hub.calls().as_slice(), [HubCall::Deliver(d)] if d.wake == ExternalWake::None));
}

#[test]
fn deliver_reports_hub_errors_as_transient() {
    let t = test_context(&FAKE_SOURCE, json!({}));
    *t.hub.fail_with.lock().unwrap() = Some("HUB spento".to_string());
    assert_eq!(
        block_on(t.ctx.deliver(delivery(ExternalWake::None))).unwrap_err(),
        ConnectorError::Transient("HUB spento".to_string())
    );
}

#[test]
fn send_as_user_requires_its_capability_and_an_own_origin() {
    let source = test_context(&FAKE_SOURCE, json!({}));
    denied(block_on(source.ctx.send_as_user(user_message(None))).unwrap_err(), "send_as_user");
    denied(block_on(source.ctx.send_as_user_files(user_message(None), Vec::new())).unwrap_err(), "send_as_user");
    assert!(source.hub.calls().is_empty());

    let contact = test_context(&FAKE_CONTACT, json!({}));
    assert!(matches!(block_on(contact.ctx.send_as_user(user_message(None))), Err(ConnectorError::Fatal(_))));
    let foreign = Some(origin("other", INSTANCE));
    assert!(matches!(block_on(contact.ctx.send_as_user(user_message(foreign))), Err(ConnectorError::Fatal(_))));
    assert!(contact.hub.calls().is_empty());

    let own = Some(origin("fake-contact", INSTANCE));
    block_on(contact.ctx.send_as_user(user_message(own.clone()))).unwrap();
    block_on(contact.ctx.send_as_user_files(user_message(own), Vec::new())).unwrap();
    assert!(matches!(contact.hub.calls().as_slice(), [HubCall::SendUser(_), HubCall::SendUserFiles(..)]));
}

#[test]
fn secrets_are_limited_to_the_declared_slots() {
    let TestContext { ctx, secrets, .. } = test_context(&FAKE_SOURCE, json!({}));
    assert_eq!(ctx.secret("api_token").unwrap(), None);
    ctx.set_secret("api_token", "tok-1").unwrap();
    ctx.set_secret("refresh_token", "ref-1").unwrap();
    assert_eq!(ctx.secret("api_token").unwrap().as_deref(), Some("tok-1"));
    assert!(crate::secrets::SecretStore::get(&*secrets, &format!("connector-{INSTANCE}-refresh_token"))
        .unwrap()
        .is_some());
    for slot in ["bot_token", "other", "", "../x"] {
        assert!(matches!(ctx.secret(slot), Err(ConnectorError::Fatal(_))), "{slot:?}");
        assert!(matches!(ctx.set_secret(slot, "x"), Err(ConnectorError::Fatal(_))), "{slot:?}");
    }
    // Un valore non valido non si salva e non compare nell'errore.
    let err = ctx.set_secret("api_token", "tok con spazi SEGRETO").unwrap_err();
    assert!(!err.to_string().contains("SEGRETO"));
    assert_eq!(ctx.secret("api_token").unwrap().as_deref(), Some("tok-1"));
}

#[test]
fn state_clock_and_randomness_are_injected() {
    let t = test_context(&FAKE_SOURCE, json!({"mailbox": "x"}));
    assert_eq!(t.ctx.config(), &json!({"mailbox": "x"}));
    assert_eq!(t.ctx.instance(), INSTANCE);
    assert_eq!(t.ctx.now(), 1_700_000_000_000);
    t.clock.advance(5);
    assert_eq!(t.ctx.now(), 1_700_000_000_005);
    assert_eq!(t.ctx.random_bytes(3).unwrap(), vec![0, 1, 2]);
    assert!(t.ctx.random_bytes(1025).is_err());
    assert_eq!(t.ctx.state_get("k"), None);
    t.ctx.state_set("k", json!(1)).unwrap();
    assert_eq!(t.ctx.state_get("k"), Some(json!(1)));
    assert_eq!(t.state.lock().custom_get("k"), Some(&json!(1)));
    assert!(t.ctx.state_set("Bad Key", json!(1)).is_err());
}

#[test]
fn the_http_client_of_the_context_only_knows_the_declared_hosts() {
    let t = test_context(&FAKE_SOURCE, json!({}));
    assert_eq!(t.ctx.http().hosts(), &["api.fake-source.example"]);
    let blocked = block_on(t.ctx.http().get("https://api.fake-contact.example/x"));
    assert!(blocked.is_err());
}

#[test]
fn errors_convert_to_connector_errors_without_leaking_urls() {
    let error = ConnectorError::from(crate::connectors::runtime::http::HttpError::Network);
    assert!(!error.to_string().contains("http"));
}

#[test]
fn the_system_clock_gives_time_and_random_bytes() {
    let clock = SystemClock;
    assert!(clock.now_ms() > 1_700_000_000_000);
    let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
    clock.fill_random(&mut a).unwrap();
    clock.fill_random(&mut b).unwrap();
    assert_ne!(a, b);
}

#[test]
fn the_hub_port_of_the_fake_reports_threads() {
    let t = test_context(&FAKE_SOURCE, json!({}));
    t.hub.add_thread(&Channel::General, "thr-1");
    assert!(block_on(t.hub.thread_exists(&Channel::General, "thr-1")).unwrap());
    assert!(!block_on(t.hub.thread_exists(&Channel::General, "thr-2")).unwrap());
}

fn item(route: &str, external_id: &str) -> SourceItem {
    SourceItem {
        route: route.to_string(),
        external_id: external_id.to_string(),
        conversation_key: Some("conv-1".to_string()),
        title: "Oggetto".to_string(),
        sender: "Mario <mario@example.com>".to_string(),
        body: "Email da Mario\n\nTesto".to_string(),
        files: Vec::new(),
    }
}

#[test]
fn a_fake_source_polls_scripted_batches_then_empty_pages() {
    use crate::connectors::testing::FakeSource;
    let source = FakeSource::new();
    let t = test_context(&FAKE_SOURCE, json!({}));
    let routes = [ActiveRoute { id: "rte-1-1".to_string(), filter: json!({"query": "x"}) }];
    source.push(Ok(PollBatch { items: vec![item("rte-1-1", "m1")], cursor: json!(10), more: true }));
    source.push(Err(ConnectorError::Transient("giù".to_string())));
    let first = block_on(source.poll(&t.ctx, &routes, None)).unwrap();
    assert_eq!((first.items.len(), first.cursor, first.more), (1, json!(10), true));
    assert_eq!(first.items[0].external_id, "m1");
    assert_eq!(block_on(source.poll(&t.ctx, &routes, Some(json!(10)))), Err(ConnectorError::Transient("giù".into())));
    let empty = block_on(source.poll(&t.ctx, &routes, Some(json!(10)))).unwrap();
    assert!(empty.items.is_empty() && !empty.more);
    assert_eq!(source.polls(), 3);
    assert!(source.validate_filter(&json!({"query": "*"})).is_err() && source.validate_filter(&json!({})).is_ok());
}

#[test]
fn a_fake_contact_receives_sends_and_answers_pairing() {
    use crate::connectors::testing::FakeContact;
    let contact = FakeContact::new();
    let t = test_context(&FAKE_CONTACT, json!({}));
    let inbound = InboundMessage {
        external_id: "7".to_string(),
        account: "42".to_string(),
        chat: "42".to_string(),
        sender_name: "Mario".to_string(),
        text: "/start CODICE".to_string(),
        reply_to_external: None,
        files: Vec::new(),
        pairing_code: Some("CODICE".to_string()),
        command: None,
    };
    contact.push(Ok(ReceiveBatch { messages: vec![inbound.clone()], cursor: json!(8) }));
    let batch = block_on(contact.receive(&t.ctx, None)).unwrap();
    assert_eq!((batch.messages, batch.cursor), (vec![inbound], json!(8)));
    let out = OutboundMessage { chat: "42".to_string(), text: "ciao".to_string(), reply_to_external: Some("7".into()) };
    assert_eq!(block_on(contact.send(&t.ctx, &out)).unwrap(), vec!["ext-1".to_string()]);
    let outcome = PairingOutcome::Linked { display_name: "Mario".to_string() };
    block_on(contact.pairing_reply(&t.ctx, &outcome, "42")).unwrap();
    assert_eq!(contact.pairing.lock().unwrap().as_slice(), &[(outcome, "42".to_string())]);
    let idle = block_on(contact.receive(&t.ctx, Some(json!(8)))).unwrap();
    assert!(idle.messages.is_empty() && idle.cursor == json!(8));
    let done = block_on(contact.action(&t.ctx, ActionId::Pair)).unwrap();
    assert_eq!(done.detail.as_deref(), Some("dettaglio"));
}

#[test]
fn a_connector_impl_exposes_its_descriptor_and_kind() {
    for connector in crate::connectors::testing::fakes() {
        assert_eq!(connector.kind(), connector.descriptor().kind);
        assert_eq!(connector.connector().descriptor().id, connector.descriptor().id);
    }
}
