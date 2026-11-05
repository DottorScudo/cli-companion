use super::session::INBOUND_PER_MINUTE;
use super::*;
use crate::connectors::contracts::{ContactCommand, RemoteFile};
use crate::connectors::model::ConnectorsFile;
use crate::connectors::testing::{inbound, test_context, FakeContact, TestContext, FAKE_CONTACT, INSTANCE};
use crate::connectors::testing::{link, new_contact, HubCall};
use crate::hub::contact::ThreadEntry;
use crate::hub::{Channel, ContactNotice, MessageOrigin};
use serde_json::json;

mod commands;
mod inbound;
mod notices;
mod pairing;

/// L'istante iniziale dell'orologio finto.
const NOW: i64 = 1_700_000_000_000;

/// Un'istanza di contatto con i suoi collegamenti. Le sequenze di messaggi girano nello
/// stesso `ContactCtx` (`run`), cosi' la memoria dell'account (`/threads` poi `/use`) resta.
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

    /// Un collegamento di `u1` con `thr-1` come thread corrente.
    fn linked() -> Self {
        Self::new(vec![link("lnk-1-1", "u1", Some("thr-1"))])
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
            session: ContactSession::default(),
        }
    }

    /// I messaggi, uno dopo l'altro, con lo stesso contesto; si ferma al primo errore.
    fn run(&self, messages: Vec<InboundMessage>) -> Result<(), ConnectorError> {
        let cc = self.cc();
        tauri::async_runtime::block_on(async {
            for message in messages {
                cc.handle_inbound(message).await?;
            }
            Ok(())
        })
    }

    fn inbound(&self, message: InboundMessage) -> Result<(), ConnectorError> {
        self.run(vec![message])
    }

    fn notice(&self, notice: &ContactNotice) -> Result<(), ConnectorError> {
        tauri::async_runtime::block_on(self.cc().send_notice(notice))
    }

    fn links(&self) -> Vec<ConnectorLink> {
        self.store.snapshot().links
    }

    /// Il collegamento `lnk-1-1`.
    fn first_link(&self) -> ConnectorLink {
        self.links().into_iter().find(|l| l.id == "lnk-1-1").expect("collegamento presente")
    }

    fn texts(&self) -> Vec<String> {
        self.contact.sent_texts()
    }
}

fn open_thread(channel: Channel, channel_name: &str, id: &str, waiting_user: bool, updated_at: i64) -> ThreadEntry {
    ThreadEntry {
        channel,
        channel_name: channel_name.to_string(),
        thread_id: id.to_string(),
        title: format!("Titolo {id}"),
        waiting_user,
        updated_at,
    }
}

#[test]
fn an_inbound_message_is_processed_once() {
    let rig = Rig::linked();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert_eq!(rig.t.hub.calls().len(), 1);
}

#[test]
fn a_duplicate_command_is_not_answered_twice() {
    let rig = Rig::linked();
    let help = crate::connectors::testing::command("c1", "u1", ContactCommand::Help);
    rig.run(vec![help.clone(), help]).unwrap();
    assert_eq!(rig.texts().len(), 1);
}

#[test]
fn unlinked_accounts_are_ignored_silently_and_their_files_are_never_fetched() {
    let rig = Rig::linked();
    let file = RemoteFile { id: "f1".to_string(), name: "a.txt".to_string(), size: Some(3) };
    let with_file = InboundMessage { files: vec![file], ..inbound("m1", "sconosciuto", "ciao") };
    let command = crate::connectors::testing::command("m2", "sconosciuto", ContactCommand::Threads);
    rig.run(vec![with_file, command]).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(rig.contact.fetched.lock().unwrap().is_empty());
    assert!(rig.contact.pairing.lock().unwrap().is_empty() && rig.contact.sent.lock().unwrap().is_empty());
}

#[test]
fn messages_over_the_rate_limit_are_dropped_and_only_the_first_drop_is_announced() {
    let rig = Rig::linked();
    let burst: Vec<InboundMessage> =
        (0..INBOUND_PER_MINUTE + 2).map(|i| inbound(&format!("m{i}"), "u1", "ciao")).collect();
    rig.run(burst).unwrap();
    assert_eq!(rig.t.hub.calls().len(), INBOUND_PER_MINUTE, "i messaggi oltre il limite non arrivano all'HUB");
    assert_eq!(rig.texts(), vec![replies::rate_limited()]);
    // Gli scartati sono gia' trattati: non tornano.
    rig.inbound(inbound(&format!("m{INBOUND_PER_MINUTE}"), "u1", "ciao")).unwrap();
    assert_eq!(rig.texts().len(), 1);
}

#[test]
fn the_rate_limit_applies_to_commands_and_is_per_account() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1")), link("lnk-1-2", "u2", Some("thr-1"))]);
    let mut burst: Vec<InboundMessage> = (0..INBOUND_PER_MINUTE)
        .map(|i| crate::connectors::testing::command(&format!("c{i}"), "u1", ContactCommand::Status))
        .collect();
    burst.push(crate::connectors::testing::command("extra", "u1", ContactCommand::Status));
    burst.push(inbound("other", "u2", "ciao"));
    rig.run(burst).unwrap();
    let texts = rig.texts();
    assert_eq!(texts.iter().filter(|t| **t == replies::rate_limited()).count(), 1);
    assert_eq!(rig.t.hub.calls().len(), 1, "l'altro account non e' limitato");
}

#[test]
fn the_window_reopens_after_a_minute() {
    let rig = Rig::linked();
    let cc = rig.cc();
    tauri::async_runtime::block_on(async {
        for i in 0..INBOUND_PER_MINUTE + 1 {
            cc.handle_inbound(inbound(&format!("a{i}"), "u1", "x")).await.unwrap();
        }
        rig.t.clock.advance(61_000);
        cc.handle_inbound(inbound("late", "u1", "x")).await.unwrap();
    });
    assert_eq!(rig.t.hub.calls().len(), INBOUND_PER_MINUTE + 1);
}

#[test]
fn a_failed_reply_is_only_logged_but_an_auth_error_stops_the_supervisor() {
    let rig = Rig::linked();
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.inbound(crate::connectors::testing::command("c1", "u1", ContactCommand::Help)).unwrap();
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    let stopped = rig.inbound(crate::connectors::testing::command("c2", "u1", ContactCommand::Help));
    assert_eq!(stopped, Err(ConnectorError::Auth("x".to_string())));
    *rig.contact.send_error.lock().unwrap() = None;
    rig.inbound(crate::connectors::testing::command("c2", "u1", ContactCommand::Help)).unwrap();
    assert_eq!(rig.texts().len(), 1, "il messaggio non trattato si ripresenta e ora riceve la risposta");
}

#[test]
fn the_origin_names_the_connector_and_the_instance() {
    let rig = Rig::linked();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    let calls = rig.t.hub.calls();
    let [HubCall::SendUser(sent)] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!(
        sent.origin,
        Some(MessageOrigin {
            connector: "fake-contact".to_string(),
            instance: INSTANCE.to_string(),
            external_id: "m1".to_string()
        })
    );
}
