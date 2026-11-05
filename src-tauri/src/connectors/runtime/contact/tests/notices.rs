use super::*;
use crate::connectors::testing::contact_notice;
use crate::hub::NoticeCategory;

#[test]
fn the_text_has_the_channel_and_thread_then_the_full_message() {
    let notice = contact_notice("msg-1", "  Prima riga\nSeconda riga  ");
    assert_eq!(super::super::notices::notice_text(&notice), "Generale › Piano\nPrima riga\nSeconda riga");
}

#[test]
fn an_empty_message_falls_back_to_the_notice_title() {
    let notice = contact_notice("msg-1", " \n ");
    assert_eq!(super::super::notices::notice_text(&notice), "Generale › Piano\nNuova risposta");
}

#[test]
fn a_notice_goes_to_every_unmuted_link_and_the_external_id_is_mapped() {
    let mut muted = link("lnk-1-2", "u2", None);
    muted.muted_until = Some(NOW + 1000);
    let mut expired = link("lnk-1-3", "u3", None);
    expired.muted_until = Some(NOW - 1000);
    let mut forever = link("lnk-1-4", "u4", None);
    forever.muted_until = Some(i64::MAX);
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None), muted, expired, forever]);
    rig.notice(&contact_notice("msg-42", "Fatto")).unwrap();
    let sent = rig.contact.sent.lock().unwrap().clone();
    let chats: Vec<&str> = sent.iter().map(|m| m.chat.as_str()).collect();
    assert_eq!(chats, ["chat-u1", "chat-u3"]);
    assert_eq!(sent[0].text, "Generale › Piano\nFatto");
    let state = rig.t.state.lock();
    assert_eq!(state.hub_to_external.get("msg-42"), Some("ext-2"));
    assert_eq!(state.external_to_hub.get("ext-1"), Some("general thr-1 msg-42"));
    assert_eq!(state.external_to_hub.get("ext-2"), Some("general thr-1 msg-42"));
}

#[test]
fn every_part_of_a_split_message_is_mapped() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    rig.contact.split_sends_in(3);
    rig.notice(&contact_notice("msg-42", "lungo")).unwrap();
    assert_eq!(rig.contact.sent.lock().unwrap().len(), 1, "il connettore riceve il messaggio intero");
    let state = rig.t.state.lock();
    for id in ["ext-1", "ext-2", "ext-3"] {
        assert_eq!(state.external_to_hub.get(id), Some("general thr-1 msg-42"), "{id}");
    }
}

#[test]
fn a_category_the_connector_refuses_sends_nothing() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    rig.contact.refuse(NoticeCategory::Reply);
    rig.notice(&contact_notice("msg-1", "x")).unwrap();
    assert!(rig.contact.sent.lock().unwrap().is_empty());
    let mut failed = contact_notice("msg-2", "x");
    failed.category = NoticeCategory::TaskFailed;
    rig.notice(&failed).unwrap();
    assert_eq!(rig.contact.sent.lock().unwrap().len(), 1);
}

#[test]
fn a_notice_without_links_sends_nothing() {
    let rig = Rig::new(Vec::new());
    rig.notice(&contact_notice("msg-1", "x")).unwrap();
    assert!(rig.contact.sent.lock().unwrap().is_empty());
}

#[test]
fn a_transient_send_error_is_logged_and_an_auth_error_propagates() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.notice(&contact_notice("msg-1", "x")).unwrap();
    assert!(rig.t.state.lock().external_to_hub.get("ext-1").is_none());
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    assert_eq!(rig.notice(&contact_notice("msg-1", "x")), Err(ConnectorError::Auth("x".to_string())));
}
