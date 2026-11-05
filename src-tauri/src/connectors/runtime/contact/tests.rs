use super::*;
use crate::connectors::contracts::PairingOutcome;
use crate::connectors::model::ConnectorsFile;
use crate::connectors::runtime::pairing::ATTEMPTS_MAX;
use crate::connectors::testing::{
    inbound, link, new_contact, test_context, FakeContact, HubCall, TestContext, FAKE_CONTACT, INSTANCE,
};
use crate::hub::{Channel, HubNoticeSound};
use serde_json::json;

struct Rig {
    t: TestContext,
    contact: &'static FakeContact,
    store: Arc<ConnectorsStore>,
    pairing: PairingRegistry,
    ids: IdGenerator,
}

impl Rig {
    fn new(links: Vec<ConnectorLink>) -> Self {
        let store = Arc::new(ConnectorsStore::default());
        store.replace(ConnectorsFile { links, ..ConnectorsFile::default() });
        let t = test_context(&FAKE_CONTACT, json!({}));
        t.hub.add_thread(&Channel::General, "thr-1");
        Self { t, contact: new_contact(), store, pairing: PairingRegistry::default(), ids: IdGenerator::default() }
    }

    fn cc(&self) -> ContactCtx<'_> {
        ContactCtx {
            connector: self.contact,
            ctx: &self.t.ctx,
            hub: &*self.t.hub,
            state: &self.t.state,
            store: &self.store,
            pairing: &self.pairing,
            ids: &self.ids,
            clock: &*self.t.clock,
        }
    }

    fn inbound(&self, message: InboundMessage) -> Result<(), ConnectorError> {
        tauri::async_runtime::block_on(self.cc().handle_inbound(message))
    }

    fn notice(&self, notice: &HubNotice) -> Result<(), ConnectorError> {
        tauri::async_runtime::block_on(self.cc().send_notice(notice))
    }

    fn links(&self) -> Vec<ConnectorLink> {
        self.store.snapshot().links
    }
}

fn hub_notice(message_id: &str) -> HubNotice {
    HubNotice {
        channel: Channel::General,
        thread_id: "thr-1".to_string(),
        message_id: message_id.to_string(),
        title: "Titolo".to_string(),
        body: "Corpo".to_string(),
        sound: HubNoticeSound::Ding,
    }
}

fn pairing_message(id: &str, account: &str, code: &str) -> InboundMessage {
    InboundMessage { pairing_code: Some(code.to_string()), ..inbound(id, account, "/start") }
}

#[test]
fn a_linked_account_writes_to_its_current_thread_with_the_origin() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    let calls = rig.t.hub.calls();
    let [HubCall::SendUser(sent)] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!((sent.thread_id.as_str(), sent.body.as_str(), sent.reply_to.clone()), ("thr-1", "ciao", None));
    assert_eq!(
        sent.origin,
        Some(MessageOrigin {
            connector: "fake-contact".to_string(),
            instance: INSTANCE.to_string(),
            external_id: "m1".to_string()
        })
    );
    // L'id esterno del messaggio entra nella mappa verso il messaggio dell'HUB.
    assert_eq!(rig.t.state.lock().external_to_hub.get("m1"), Some("msg-1"));
}

#[test]
fn a_reply_to_a_known_message_carries_reply_to_and_an_unknown_one_does_not() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    rig.t.state.lock().link_messages("ext-notice", "msg-77");
    let reply = InboundMessage { reply_to_external: Some("ext-notice".to_string()), ..inbound("m1", "u1", "si") };
    let stale = InboundMessage { reply_to_external: Some("ext-vecchio".to_string()), ..inbound("m2", "u1", "no") };
    rig.inbound(reply).unwrap();
    rig.inbound(stale).unwrap();
    let replies: Vec<Option<String>> = rig
        .t
        .hub
        .calls()
        .into_iter()
        .map(|c| match c {
            HubCall::SendUser(m) => m.reply_to,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(replies, vec![Some("msg-77".to_string()), None]);
}

#[test]
fn attachments_go_through_send_user_files() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    let file = crate::hub::attachments::IncomingFile { name: "a.txt".to_string(), bytes: vec![1, 2] };
    rig.inbound(InboundMessage { files: vec![file], ..inbound("m1", "u1", "") }).unwrap();
    assert!(matches!(rig.t.hub.calls().as_slice(), [HubCall::SendUserFiles(_, files)] if files.len() == 1));
}

#[test]
fn unlinked_accounts_are_ignored_silently() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    rig.inbound(inbound("m1", "sconosciuto", "ciao")).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(rig.contact.pairing.lock().unwrap().is_empty() && rig.contact.sent.lock().unwrap().is_empty());
}

#[test]
fn a_link_without_a_current_thread_is_skipped_and_a_vanished_thread_clears_it() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None), link("lnk-1-2", "u2", Some("thr-sparito"))]);
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    rig.inbound(inbound("m2", "u2", "ciao")).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(rig.links().iter().all(|l| l.current.is_none()));
}

#[test]
fn an_inbound_message_is_processed_once() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert_eq!(rig.t.hub.calls().len(), 1);
}

#[test]
fn a_hub_failure_is_reported_and_the_message_stays_unseen_for_the_next_try() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    *rig.t.hub.fail_with.lock().unwrap() = Some("HUB spento".to_string());
    assert_eq!(rig.inbound(inbound("m1", "u1", "ciao")), Err(ConnectorError::Transient("HUB spento".to_string())));
    *rig.t.hub.fail_with.lock().unwrap() = None;
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert_eq!(rig.t.hub.calls().len(), 2);
}

#[test]
fn a_valid_code_links_the_account_and_the_connector_replies() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "u1", &code)).unwrap();
    let links = rig.links();
    assert_eq!(links.len(), 1);
    assert_eq!((links[0].external_account.as_str(), links[0].external_chat.as_str()), ("u1", "chat-u1"));
    assert!(links[0].current.is_none() && links[0].id.starts_with("lnk-"));
    assert_eq!(
        *rig.contact.pairing.lock().unwrap(),
        vec![(PairingOutcome::Linked { display_name: "u1".to_string() }, "chat-u1".to_string())]
    );
    // Lo stesso account che si riabbina non duplica il collegamento.
    let again = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m2", "u1", &again)).unwrap();
    assert_eq!(rig.links().len(), 1);
}

#[test]
fn a_wrong_or_reused_code_is_rejected_and_creates_nothing() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "u1", "SBAGLIATO123")).unwrap();
    rig.inbound(pairing_message("m2", "u1", &code)).unwrap();
    rig.inbound(pairing_message("m3", "u2", &code)).unwrap();
    assert_eq!(rig.links().len(), 1, "solo il secondo tentativo, col codice giusto");
    let outcomes: Vec<PairingOutcome> = rig.contact.pairing.lock().unwrap().iter().map(|(o, _)| o.clone()).collect();
    assert_eq!(outcomes[0], PairingOutcome::Rejected);
    assert!(matches!(outcomes[1], PairingOutcome::Linked { .. }));
    assert_eq!(outcomes[2], PairingOutcome::Rejected, "il codice e' monouso");
}

#[test]
fn an_expired_code_is_rejected() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.t.clock.advance(11 * 60 * 1000);
    rig.inbound(pairing_message("m1", "u1", &code)).unwrap();
    assert!(rig.links().is_empty());
    assert_eq!(rig.contact.pairing.lock().unwrap()[0].0, PairingOutcome::Rejected);
}

#[test]
fn too_many_attempts_stop_even_the_right_code() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    for i in 0..ATTEMPTS_MAX {
        rig.inbound(pairing_message(&format!("w{i}"), "u9", "SBAGLIATO123")).unwrap();
    }
    rig.inbound(pairing_message("giusto", "u1", &code)).unwrap();
    assert!(rig.links().is_empty());
    assert_eq!(rig.contact.pairing.lock().unwrap().last().unwrap().0, PairingOutcome::TooManyAttempts);
}

#[test]
fn the_link_limit_per_instance_is_respected() {
    let existing: Vec<ConnectorLink> = (1..=5).map(|i| link(&format!("lnk-1-{i}"), &format!("u{i}"), None)).collect();
    let rig = Rig::new(existing);
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "nuovo", &code)).unwrap();
    assert_eq!(rig.links().len(), 5);
    assert_eq!(rig.contact.pairing.lock().unwrap()[0].0, PairingOutcome::Rejected);
}

#[test]
fn a_notice_goes_to_every_unmuted_link_and_the_external_id_is_mapped() {
    let mut muted = link("lnk-1-2", "u2", None);
    muted.muted_until = Some(rig_now() + 1000);
    let mut expired = link("lnk-1-3", "u3", None);
    expired.muted_until = Some(rig_now() - 1000);
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None), muted, expired]);
    rig.notice(&hub_notice("msg-42")).unwrap();
    let sent = rig.contact.sent.lock().unwrap().clone();
    let chats: Vec<&str> = sent.iter().map(|m| m.chat.as_str()).collect();
    assert_eq!(chats, vec!["chat-u1", "chat-u3"]);
    assert_eq!(sent[0].text, "Titolo\nCorpo");
    assert_eq!(rig.t.state.lock().hub_to_external.get("msg-42"), Some("ext-2"));
    assert_eq!(rig.t.state.lock().external_to_hub.get("ext-1"), Some("msg-42"));
}

fn rig_now() -> i64 {
    1_700_000_000_000
}

#[test]
fn a_notice_without_links_sends_nothing() {
    let rig = Rig::new(Vec::new());
    rig.notice(&hub_notice("msg-1")).unwrap();
    assert!(rig.contact.sent.lock().unwrap().is_empty());
}
