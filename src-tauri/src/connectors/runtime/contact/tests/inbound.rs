use super::*;
use crate::connectors::model::LinkThread;
use crate::connectors::testing::contact_notice;
use crate::hub::attachments::{ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn reply_to(id: &str, external: &str) -> InboundMessage {
    InboundMessage { reply_to_external: Some(external.to_string()), ..inbound(id, "u1", "si") }
}

fn remote(id: &str, size: Option<u64>) -> RemoteFile {
    RemoteFile { id: id.to_string(), name: format!("{id}.bin"), size }
}

fn with_files(id: &str, text: &str, files: Vec<RemoteFile>) -> InboundMessage {
    InboundMessage { files, ..inbound(id, "u1", text) }
}

fn sent_users(rig: &Rig) -> Vec<(Channel, String, Option<String>)> {
    rig.t
        .hub
        .calls()
        .into_iter()
        .map(|call| match call {
            HubCall::SendUser(m) | HubCall::SendUserFiles(m, _) => (m.channel, m.thread_id, m.reply_to),
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_message_goes_to_the_current_thread_and_is_mapped_to_its_hub_message() {
    let rig = Rig::linked();
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    let calls = rig.t.hub.calls();
    let [HubCall::SendUser(sent)] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!((sent.thread_id.as_str(), sent.body.as_str(), sent.reply_to.clone()), ("thr-1", "ciao", None));
    let state = rig.t.state.lock();
    assert_eq!(state.external_to_hub.get("m1"), Some("general thr-1 msg-1"));
    assert_eq!(state.hub_to_external.get("msg-1"), Some("m1"));
    assert!(rig.texts().is_empty());
}

#[test]
fn a_reply_to_a_mapped_notification_writes_in_its_thread_and_makes_it_current() {
    let rig = Rig::linked();
    rig.t.hub.add_thread(&ws(), "thr-9");
    rig.t.state.lock().external_to_hub.insert("ext-5", "ws-1 thr-9 msg-77");
    rig.inbound(reply_to("m1", "ext-5")).unwrap();
    assert_eq!(sent_users(&rig), vec![(ws(), "thr-9".to_string(), Some("msg-77".to_string()))]);
    assert_eq!(rig.first_link().current, Some(LinkThread { channel: ws(), thread_id: "thr-9".to_string() }));
    assert!(rig.texts().is_empty());
    // Il messaggio successivo, non una risposta, segue il nuovo thread corrente.
    rig.inbound(inbound("m2", "u1", "e poi")).unwrap();
    assert_eq!(sent_users(&rig)[1], (ws(), "thr-9".to_string(), None));
}

#[test]
fn an_end_to_end_reply_to_a_sent_notice_reaches_the_noticed_thread() {
    let rig = Rig::linked();
    rig.notice(&contact_notice("msg-42", "Fatto")).unwrap();
    rig.inbound(reply_to("m1", "ext-1")).unwrap();
    assert_eq!(sent_users(&rig), vec![(Channel::General, "thr-1".to_string(), Some("msg-42".to_string()))]);
}

#[test]
fn a_reply_to_a_message_that_is_unmapped_or_legacy_goes_to_the_current_thread_with_a_note() {
    let rig = Rig::linked();
    rig.t.state.lock().link_messages("ext-old", "msg-77");
    rig.run(vec![reply_to("m1", "ext-sconosciuto"), reply_to("m2", "ext-old")]).unwrap();
    assert_eq!(sent_users(&rig), vec![(Channel::General, "thr-1".to_string(), None); 2]);
    assert_eq!(rig.texts(), vec![replies::stale_reply(); 2]);
}

#[test]
fn a_mapped_reply_whose_thread_is_gone_falls_back_to_the_current_thread() {
    let rig = Rig::linked();
    rig.t.state.lock().external_to_hub.insert("ext-5", "ws-1 thr-9 msg-77");
    rig.inbound(reply_to("m1", "ext-5")).unwrap();
    assert_eq!(sent_users(&rig), vec![(Channel::General, "thr-1".to_string(), None)]);
    assert_eq!(rig.texts(), vec![replies::stale_reply()]);
    assert_eq!(rig.first_link().current.map(|c| c.thread_id), Some("thr-1".to_string()));
}

#[test]
fn without_a_current_thread_the_user_is_told_how_to_pick_one_and_nothing_is_fetched() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    rig.contact.script_file("f1", Ok(vec![1]));
    rig.inbound(with_files("m1", "ciao", vec![remote("f1", Some(1))])).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(rig.contact.fetched.lock().unwrap().is_empty());
    assert_eq!(rig.texts(), vec![replies::no_current_thread()]);
    assert!(rig.t.state.lock().external_to_hub.get("m1").is_none());
}

#[test]
fn a_vanished_current_thread_is_cleared_and_the_user_is_told() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-sparito"))]);
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert!(rig.first_link().current.is_none());
    assert_eq!(rig.texts(), vec![replies::current_thread_gone()]);
}

#[test]
fn a_hub_error_is_reported_once_and_the_message_is_treated_as_handled() {
    let rig = Rig::linked();
    *rig.t.hub.fail_with.lock().unwrap() = Some("HUB spento".to_string());
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert_eq!(rig.texts(), vec![replies::not_delivered("HUB spento")]);
    *rig.t.hub.fail_with.lock().unwrap() = None;
    rig.inbound(inbound("m1", "u1", "ciao")).unwrap();
    assert_eq!(rig.t.hub.calls().len(), 1, "non si riprova");
    assert!(rig.t.state.lock().external_to_hub.get("m1").is_none());
}

#[test]
fn a_rejected_citation_is_retried_without_it() {
    let rig = Rig::linked();
    rig.t.state.lock().external_to_hub.insert("ext-5", "general thr-1 msg-77");
    rig.t.hub.fail_at.lock().unwrap().push(1);
    rig.inbound(reply_to("m1", "ext-5")).unwrap();
    assert_eq!(
        sent_users(&rig),
        vec![
            (Channel::General, "thr-1".to_string(), Some("msg-77".to_string())),
            (Channel::General, "thr-1".to_string(), None)
        ]
    );
    assert!(rig.texts().is_empty());
    assert!(rig.t.state.lock().external_to_hub.get("m1").is_some());
}

#[test]
fn files_within_the_limits_are_fetched_and_sent_with_the_message() {
    let rig = Rig::linked();
    rig.contact.script_file("f1", Ok(vec![1, 2]));
    rig.inbound(with_files("m1", "", vec![remote("f1", Some(2))])).unwrap();
    let calls = rig.t.hub.calls();
    let [HubCall::SendUserFiles(_, files)] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!((files[0].name.as_str(), files[0].bytes.clone()), ("f1.bin", vec![1, 2]));
    assert!(rig.texts().is_empty());
}

#[test]
fn files_over_the_limits_are_skipped_with_one_reply_and_the_text_still_goes() {
    let rig = Rig::linked();
    rig.contact.script_file("ok", Ok(vec![7]));
    rig.contact.script_file("grande", Ok(vec![0; ATTACHMENT_SIZE_MAX as usize + 1]));
    let files = vec![
        remote("dichiarato", Some(ATTACHMENT_SIZE_MAX + 1)),
        remote("grande", None),
        remote("rotto", Some(1)),
        remote("ok", Some(1)),
    ];
    rig.inbound(with_files("m1", "guarda", files)).unwrap();
    assert_eq!(
        *rig.contact.fetched.lock().unwrap(),
        ["grande", "rotto", "ok"],
        "quello dichiarato troppo grande non si scarica"
    );
    let calls = rig.t.hub.calls();
    let [HubCall::SendUserFiles(user, sent)] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!((user.body.as_str(), sent.len()), ("guarda", 1));
    assert_eq!(
        rig.texts(),
        vec![
            "Allegati non importati: dichiarato.bin (troppo grande), grande.bin (troppo grande), rotto.bin (download non riuscito). Limiti: 10 MB per file, 5 per messaggio."
        ]
    );
}

#[test]
fn only_the_first_files_up_to_the_cap_are_fetched() {
    let rig = Rig::linked();
    let files: Vec<RemoteFile> = (0..ATTACHMENTS_MAX + 1).map(|i| remote(&format!("f{i}"), Some(1))).collect();
    for file in &files {
        rig.contact.script_file(&file.id, Ok(vec![1]));
    }
    rig.inbound(with_files("m1", "", files)).unwrap();
    assert_eq!(rig.contact.fetched.lock().unwrap().len(), ATTACHMENTS_MAX);
    assert!(rig.texts()[0].contains(&format!("f{ATTACHMENTS_MAX}.bin (troppi allegati)")));
}

#[test]
fn nothing_is_sent_when_no_text_and_no_file_is_left() {
    let rig = Rig::linked();
    rig.inbound(inbound("m1", "u1", "   ")).unwrap();
    assert!(rig.t.hub.calls().is_empty() && rig.texts().is_empty());
    rig.inbound(with_files("m2", " ", vec![remote("rotto", Some(1))])).unwrap();
    assert!(rig.t.hub.calls().is_empty());
    assert_eq!(rig.texts().len(), 1, "solo l'avviso degli allegati non importati");
}

#[test]
fn an_auth_error_while_fetching_stops_the_supervisor_before_anything_is_written() {
    let rig = Rig::linked();
    rig.contact.script_file("f1", Err(ConnectorError::Auth("x".to_string())));
    let result = rig.inbound(with_files("m1", "ciao", vec![remote("f1", Some(1))]));
    assert_eq!(result, Err(ConnectorError::Auth("x".to_string())));
    assert!(rig.t.hub.calls().is_empty());
}
