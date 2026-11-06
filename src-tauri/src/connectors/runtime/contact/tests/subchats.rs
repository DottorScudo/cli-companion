//! Le sotto-chat da capo a fondo, con il connettore finto: una chat `group-1` con le
//! sotto-chat, collegata all'account `u1`.

use super::*;
use crate::connectors::contracts::PairingOutcome;
use crate::connectors::model::LinkThread;
use crate::connectors::runtime::contact::subchats::{SubchatKey, ThreadKey};
use crate::connectors::testing::{command, contact_notice};
use crate::hub::contact::ChannelEntry;

mod lifecycle;
mod mirror;

const GROUP: &str = "group-1";

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn group_link(id: &str, account: &str, current: Option<&str>) -> ConnectorLink {
    ConnectorLink { external_chat: GROUP.to_string(), ..link(id, account, current) }
}

/// `u1` collegato nel gruppo, con `thr-1` come thread corrente.
fn group_rig() -> Rig {
    group_rig_with(vec![group_link("lnk-1-1", "u1", Some("thr-1"))])
}

fn group_rig_with(links: Vec<ConnectorLink>) -> Rig {
    let rig = Rig::new(links);
    rig.contact.with_subchats_in(GROUP);
    rig
}

/// Un messaggio di `account` nel gruppo, nella sotto-chat `subchat` (`None` = la principale).
fn in_group(id: &str, account: &str, subchat: Option<&str>, text: &str) -> InboundMessage {
    InboundMessage { chat: GROUP.to_string(), subchat: subchat.map(str::to_string), ..inbound(id, account, text) }
}

fn group_command(id: &str, subchat: Option<&str>, c: ContactCommand) -> InboundMessage {
    InboundMessage { chat: GROUP.to_string(), subchat: subchat.map(str::to_string), ..command(id, "u1", c) }
}

fn thread(channel: Channel, id: &str) -> ThreadKey {
    ThreadKey { channel, thread_id: id.to_string() }
}

fn key(subchat: &str) -> SubchatKey {
    SubchatKey { chat: GROUP.to_string(), subchat: subchat.to_string() }
}

/// Lega la sotto-chat `subchat` del gruppo al thread.
fn map_subchat(rig: &Rig, thread: ThreadKey, subchat: &str) {
    assert!(rig.t.state.lock().subchats.insert(thread, key(subchat)));
}

fn mapped_thread(rig: &Rig, subchat: &str) -> Option<ThreadKey> {
    rig.t.state.lock().subchats.thread_of(&key(subchat))
}

/// I messaggi mandati: `(chat, sotto-chat, testo)`.
fn sent(rig: &Rig) -> Vec<(String, Option<String>, String)> {
    rig.contact.sent.lock().unwrap().iter().map(|m| (m.chat.clone(), m.subchat.clone(), m.text.clone())).collect()
}

fn to(subchat: Option<&str>, text: &str) -> (String, Option<String>, String) {
    (GROUP.to_string(), subchat.map(str::to_string), text.to_string())
}

/// I messaggi scritti nell'HUB: `(canale, thread, risposta a)`.
fn written(rig: &Rig) -> Vec<(Channel, String, Option<String>)> {
    rig.t
        .hub
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            HubCall::SendUser(m) | HubCall::SendUserFiles(m, _) => Some((m.channel, m.thread_id, m.reply_to)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_notice_opens_the_subchat_once_reuses_it_and_drops_the_prefix() {
    let rig = group_rig();
    rig.notice(&contact_notice("msg-1", "Fatto")).unwrap();
    rig.notice(&contact_notice("msg-2", "Ancora")).unwrap();
    assert_eq!(*rig.contact.opened.lock().unwrap(), vec![(GROUP.to_string(), "Generale › Piano".to_string())]);
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Fatto"), to(Some("sub-1"), "Ancora")]);
    assert_eq!(mapped_thread(&rig, "sub-1"), Some(thread(Channel::General, "thr-1")));
    assert_eq!(rig.t.state.lock().external_to_hub.get("ext-2"), Some("general thr-1 msg-2"));
}

#[test]
fn links_sharing_a_chat_receive_the_notice_once() {
    let rig = group_rig_with(vec![group_link("lnk-1-1", "u1", None), group_link("lnk-1-2", "u2", None)]);
    rig.notice(&contact_notice("msg-1", "Fatto")).unwrap();
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Fatto")]);
}

#[test]
fn a_failed_open_falls_back_to_the_main_chat_with_the_prefix() {
    let rig = group_rig();
    *rig.contact.open_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.notice(&contact_notice("msg-1", "Fatto")).unwrap();
    assert_eq!(sent(&rig), vec![to(None, "Generale › Piano\nFatto")]);
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
    assert_eq!(rig.t.state.lock().external_to_hub.get("ext-1"), Some("general thr-1 msg-1"));
}

#[test]
fn an_open_error_that_stops_the_supervisor_propagates() {
    let rig = group_rig();
    *rig.contact.open_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    assert_eq!(rig.notice(&contact_notice("msg-1", "Fatto")), Err(ConnectorError::Auth("x".to_string())));
    assert!(sent(&rig).is_empty());
}

#[test]
fn an_invalid_subchat_id_from_the_service_is_not_saved() {
    let rig = group_rig();
    rig.contact.script_subchat_id("a b");
    rig.notice(&contact_notice("msg-1", "Fatto")).unwrap();
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
    assert_eq!(sent(&rig), vec![to(None, "Generale › Piano\nFatto")]);
}

#[test]
fn a_subchat_deleted_in_the_service_is_recreated_once() {
    let rig = group_rig();
    rig.notice(&contact_notice("msg-1", "Primo")).unwrap();
    rig.contact.delete_remotely("sub-1");
    rig.notice(&contact_notice("msg-2", "Secondo")).unwrap();
    assert_eq!(rig.contact.opened_titles().len(), 2);
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Primo"), to(Some("sub-2"), "Secondo")]);
    assert_eq!(mapped_thread(&rig, "sub-1"), None, "la voce morta e' tolta");
    assert_eq!(mapped_thread(&rig, "sub-2"), Some(thread(Channel::General, "thr-1")));
}

#[test]
fn a_subchat_gone_again_after_the_recreation_falls_back_to_the_main_chat() {
    let rig = group_rig();
    rig.notice(&contact_notice("msg-1", "Primo")).unwrap();
    rig.contact.delete_remotely("sub-1");
    rig.contact.delete_remotely("sub-2");
    rig.notice(&contact_notice("msg-2", "Secondo")).unwrap();
    assert_eq!(rig.contact.opened_titles().len(), 2, "ricreata una volta sola");
    assert_eq!(sent(&rig)[1], to(None, "Generale › Piano\nSecondo"));
}

#[test]
fn a_message_in_a_mapped_subchat_goes_to_its_thread_without_a_current_thread() {
    let rig = group_rig_with(vec![group_link("lnk-1-1", "u1", None)]);
    rig.t.hub.add_thread(&ws(), "thr-9");
    map_subchat(&rig, thread(ws(), "thr-9"), "sub-7");
    rig.inbound(in_group("m1", "u1", Some("sub-7"), "ciao")).unwrap();
    assert_eq!(written(&rig), vec![(ws(), "thr-9".to_string(), None)]);
    assert_eq!(rig.first_link().current, None, "il thread corrente non serve e non cambia");
    assert!(sent(&rig).is_empty());
    assert_eq!(rig.t.state.lock().external_to_hub.get("m1"), Some("ws-1 thr-9 msg-1"));
}

#[test]
fn a_reply_in_a_subchat_quotes_only_a_message_of_the_same_thread() {
    let rig = group_rig();
    rig.t.hub.add_thread(&ws(), "thr-9");
    map_subchat(&rig, thread(ws(), "thr-9"), "sub-7");
    {
        let mut state = rig.t.state.lock();
        state.external_to_hub.insert("ext-same", "ws-1 thr-9 msg-77");
        state.external_to_hub.insert("ext-other", "general thr-1 msg-5");
    }
    let reply = |id: &str, external: &str| InboundMessage {
        reply_to_external: Some(external.to_string()),
        ..in_group(id, "u1", Some("sub-7"), "si")
    };
    rig.run(vec![reply("m1", "ext-same"), reply("m2", "ext-other")]).unwrap();
    assert_eq!(
        written(&rig),
        vec![(ws(), "thr-9".to_string(), Some("msg-77".to_string())), (ws(), "thr-9".to_string(), None)]
    );
    assert_eq!(rig.first_link().current, Some(LinkThread { channel: Channel::General, thread_id: "thr-1".into() }));
    assert!(sent(&rig).is_empty(), "nessun avviso di risposta scollegata nella sotto-chat");
}

#[test]
fn a_message_in_an_unmapped_subchat_is_explained_there_and_saves_nothing() {
    let rig = group_rig();
    rig.inbound(in_group("m1", "u1", Some("sub-99"), "ciao")).unwrap();
    assert!(written(&rig).is_empty());
    assert_eq!(sent(&rig), vec![to(Some("sub-99"), &replies::subchat_unmapped())]);
    assert!(rig.t.state.lock().external_to_hub.get("m1").is_none());
}

#[test]
fn a_subchat_whose_thread_is_gone_is_explained_and_forgotten() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-sparito"), "sub-7");
    rig.inbound(in_group("m1", "u1", Some("sub-7"), "ciao")).unwrap();
    assert!(written(&rig).is_empty());
    assert_eq!(sent(&rig), vec![to(Some("sub-7"), &replies::subchat_thread_gone())]);
    assert_eq!(mapped_thread(&rig, "sub-7"), None);
}

#[test]
fn the_main_subchat_keeps_todays_rules() {
    let rig = group_rig();
    rig.notice(&contact_notice("msg-42", "Fatto")).unwrap();
    let reply = InboundMessage { reply_to_external: Some("ext-1".to_string()), ..in_group("m1", "u1", None, "si") };
    rig.run(vec![in_group("m0", "u1", None, "al corrente"), reply]).unwrap();
    assert_eq!(
        written(&rig),
        vec![
            (Channel::General, "thr-1".to_string(), None),
            (Channel::General, "thr-1".to_string(), Some("msg-42".to_string()))
        ]
    );
}

#[test]
fn other_accounts_writing_in_the_group_are_ignored_silently() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    let others = vec![
        in_group("m1", "intruso", Some("sub-1"), "ciao"),
        in_group("m2", "intruso", None, "ciao"),
        InboundMessage { chat: GROUP.to_string(), ..command("m3", "intruso", ContactCommand::Threads) },
    ];
    rig.run(others).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(sent(&rig).is_empty());
}

#[test]
fn an_invalid_subchat_id_in_a_message_is_ignored() {
    let rig = group_rig();
    rig.inbound(in_group("m1", "u1", Some("a/b"), "ciao")).unwrap();
    assert!(rig.t.hub.calls().is_empty() && sent(&rig).is_empty());
}

#[test]
fn a_subchat_from_a_chat_without_subchats_is_treated_as_the_main_chat() {
    let rig = Rig::linked();
    let message = InboundMessage { subchat: Some("sub-1".to_string()), ..inbound("m1", "u1", "ciao") };
    rig.inbound(message).unwrap();
    assert_eq!(written(&rig), vec![(Channel::General, "thr-1".to_string(), None)]);
}

#[test]
fn new_creates_the_thread_with_its_subchat() {
    let rig = group_rig();
    rig.t.hub.set_channels(vec![ChannelEntry { channel: Channel::General, name: "Generale".to_string() }]);
    rig.inbound(group_command("c1", None, ContactCommand::New("Piano B".to_string()))).unwrap();
    assert_eq!(rig.contact.opened_titles(), vec!["Generale › Piano B".to_string()]);
    assert_eq!(sent(&rig), vec![to(None, &replies::subchat_thread_created("Generale › Piano B"))]);
    assert_eq!(mapped_thread(&rig, "sub-1"), Some(thread(Channel::General, "thr-new-1")));
    rig.inbound(in_group("m1", "u1", Some("sub-1"), "primo messaggio")).unwrap();
    assert_eq!(written(&rig), vec![(Channel::General, "thr-new-1".to_string(), None)]);
}

#[test]
fn new_inside_a_subchat_still_creates_a_new_thread_and_answers_there() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-7");
    rig.inbound(group_command("c1", Some("sub-7"), ContactCommand::New("Altro".to_string()))).unwrap();
    assert_eq!(rig.contact.opened_titles(), vec!["Altro".to_string()], "senza nome del canale, solo il titolo");
    assert_eq!(sent(&rig), vec![to(Some("sub-7"), &replies::subchat_thread_created("Altro"))]);
    assert_eq!(mapped_thread(&rig, "sub-1"), Some(thread(Channel::General, "thr-new-1")));
}

#[test]
fn new_without_a_subchat_says_so_and_keeps_the_thread_current() {
    let rig = group_rig();
    *rig.contact.open_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.inbound(group_command("c1", None, ContactCommand::New("Piano B".to_string()))).unwrap();
    assert_eq!(sent(&rig), vec![to(None, &replies::thread_created_without_subchat("Piano B"))]);
    assert_eq!(rig.first_link().current.map(|c| c.thread_id), Some("thr-new-1".to_string()));
}

#[test]
fn use_opens_or_finds_the_subchat_instead_of_changing_the_current_thread() {
    let rig = group_rig();
    rig.t.hub.add_open_thread(open_thread(ws(), "Sito", "thr-9", false, 20));
    rig.run(vec![
        group_command("c1", None, ContactCommand::Threads),
        group_command("c2", None, ContactCommand::Use("1".to_string())),
        group_command("c3", Some("sub-1"), ContactCommand::Use("1".to_string())),
    ])
    .unwrap();
    assert_eq!(rig.contact.opened_titles(), vec!["Sito › Titolo thr-9".to_string()], "la seconda volta la ritrova");
    let texts = rig.texts();
    assert_eq!(texts[1], replies::subchat_ready("Sito › Titolo thr-9"));
    assert_eq!(sent(&rig)[2], to(Some("sub-1"), &replies::subchat_ready("Sito › Titolo thr-9")));
    assert_eq!(rig.first_link().current, Some(LinkThread { channel: Channel::General, thread_id: "thr-1".into() }));
}

#[test]
fn use_falls_back_to_the_current_thread_when_the_subchat_does_not_open() {
    let rig = group_rig();
    rig.t.hub.add_open_thread(open_thread(ws(), "Sito", "thr-9", false, 20));
    *rig.contact.open_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.run(vec![
        group_command("c1", None, ContactCommand::Threads),
        group_command("c2", None, ContactCommand::Use("1".to_string())),
    ])
    .unwrap();
    assert_eq!(rig.texts()[1], replies::now_using("Sito › Titolo thr-9"));
    assert_eq!(rig.first_link().current, Some(LinkThread { channel: ws(), thread_id: "thr-9".into() }));
}

#[test]
fn help_and_unknown_commands_in_a_group_explain_the_subchats() {
    let rig = group_rig();
    rig.run(vec![
        group_command("c1", None, ContactCommand::Help),
        group_command("c2", Some("sub-3"), ContactCommand::Unknown),
    ])
    .unwrap();
    assert_eq!(
        sent(&rig),
        vec![to(None, &replies::subchat_help()), to(Some("sub-3"), &replies::unknown_subchat_command())]
    );
}

#[test]
fn pairing_from_the_group_moves_the_private_link() {
    let rig = group_rig_with(vec![link("lnk-1-1", "u1", Some("thr-1"))]);
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    let pairing = InboundMessage { pairing_code: Some(code), ..in_group("m1", "u1", None, "/start") };
    rig.inbound(pairing).unwrap();
    let links = rig.links();
    assert_eq!(links.len(), 1);
    assert_eq!((links[0].id.as_str(), links[0].external_chat.as_str()), ("lnk-1-1", GROUP));
    assert_eq!(links[0].current, Some(LinkThread { channel: Channel::General, thread_id: "thr-1".into() }));
    assert_eq!(
        *rig.contact.pairing.lock().unwrap(),
        vec![(PairingOutcome::Linked { display_name: "u1".to_string() }, GROUP.to_string())]
    );
    assert_eq!(sent(&rig), vec![to(None, &replies::subchat_help())]);
}

#[test]
fn a_message_from_the_old_private_chat_gets_a_warning_and_is_not_saved() {
    let rig = group_rig();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert_eq!(sent(&rig), vec![("chat-u1".to_string(), None, replies::wrong_chat())]);
    assert!(rig.t.state.lock().external_to_hub.get("m1").is_none());
}
