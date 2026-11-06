//! L'eco dei messaggi scritti nell'app: solo nella sotto-chat che il thread ha gia', in
//! una chat con le sotto-chat, con l'interruttore acceso e per i collegamenti non silenziati.

use super::*;
use crate::connectors::testing::{app_message, SubchatOp};
use crate::hub::AppMessage;

fn mirror(rig: &Rig, message: &AppMessage) -> Result<(), ConnectorError> {
    tauri::async_runtime::block_on(rig.cc().mirror_app_message(message))
}

/// Un rig di gruppo in cui `thr-1` ha gia' il suo topic `sub-1`.
fn rig_with_topic() -> Rig {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    rig
}

#[test]
fn an_app_message_goes_to_the_existing_topic_with_the_prefix() {
    let rig = rig_with_topic();
    mirror(&rig, &app_message("msg-9", "Ciao a tutti", 0)).unwrap();
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Tu (app): Ciao a tutti")]);
    assert!(rig.contact.opened.lock().unwrap().is_empty());
}

#[test]
fn a_reply_to_the_echo_quotes_the_hub_message() {
    let rig = rig_with_topic();
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert_eq!(rig.t.state.lock().external_to_hub.get("ext-1"), Some("general thr-1 msg-9"));
    let reply =
        InboundMessage { reply_to_external: Some("ext-1".to_string()), ..in_group("m1", "u1", Some("sub-1"), "ok") };
    rig.inbound(reply).unwrap();
    assert_eq!(written(&rig), vec![(Channel::General, "thr-1".to_string(), Some("msg-9".to_string()))]);
}

#[test]
fn every_part_of_a_split_echo_is_mapped() {
    let rig = rig_with_topic();
    rig.contact.split_sends_in(2);
    mirror(&rig, &app_message("msg-9", "lungo", 0)).unwrap();
    let state = rig.t.state.lock();
    for id in ["ext-1", "ext-2"] {
        assert_eq!(state.external_to_hub.get(id), Some("general thr-1 msg-9"), "{id}");
    }
}

#[test]
fn a_thread_without_a_topic_gets_no_echo_and_no_topic() {
    let rig = group_rig();
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
    assert!(rig.contact.opened.lock().unwrap().is_empty());
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
}

#[test]
fn only_the_topic_of_the_written_thread_gets_the_echo() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-2"), "sub-2");
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
}

#[test]
fn with_the_switch_off_nothing_is_sent() {
    let rig = rig_with_topic();
    rig.contact.refuse_mirroring();
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
}

#[test]
fn a_private_chat_gets_no_echo() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
}

#[test]
fn a_muted_link_gets_no_echo() {
    let mut muted = group_link("lnk-1-1", "u1", None);
    muted.muted_until = Some(NOW + 1000);
    let rig = group_rig_with(vec![muted]);
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
}

#[test]
fn links_sharing_a_chat_get_the_echo_once() {
    let rig = group_rig_with(vec![group_link("lnk-1-1", "u1", None), group_link("lnk-1-2", "u2", None)]);
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Tu (app): Ciao")]);
}

#[test]
fn attachments_are_cited_by_count_only() {
    let rig = rig_with_topic();
    mirror(&rig, &app_message("msg-1", "Guarda", 2)).unwrap();
    mirror(&rig, &app_message("msg-2", "  ", 1)).unwrap();
    assert_eq!(
        sent(&rig),
        vec![to(Some("sub-1"), "Tu (app): Guarda\n📎 2 allegati"), to(Some("sub-1"), "Tu (app): 📎 1 allegato")]
    );
}

#[test]
fn an_empty_message_without_attachments_sends_nothing() {
    let rig = rig_with_topic();
    mirror(&rig, &app_message("msg-1", " \n ", 0)).unwrap();
    assert!(sent(&rig).is_empty());
}

#[test]
fn a_closed_topic_is_reopened_before_the_echo() {
    let rig = rig_with_topic();
    rig.t.state.lock().subchats.set_closed(&key("sub-1"), true);
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert_eq!(rig.contact.lifecycle_calls(SubchatOp::Reopen), vec![(GROUP.to_string(), "sub-1".to_string())]);
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Tu (app): Ciao")]);
}

#[test]
fn a_topic_deleted_in_the_service_is_forgotten_without_recreating_it() {
    let rig = rig_with_topic();
    rig.contact.delete_remotely("sub-1");
    mirror(&rig, &app_message("msg-9", "Ciao", 0)).unwrap();
    assert!(sent(&rig).is_empty());
    assert!(rig.contact.opened.lock().unwrap().is_empty());
    assert_eq!(mapped_thread(&rig, "sub-1"), None);
}

#[test]
fn a_failed_send_is_only_logged_but_an_auth_error_stops_the_supervisor() {
    let rig = rig_with_topic();
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    mirror(&rig, &app_message("msg-1", "Ciao", 0)).unwrap();
    assert!(rig.t.state.lock().external_to_hub.get("ext-1").is_none());
    *rig.contact.send_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    assert_eq!(mirror(&rig, &app_message("msg-2", "Ciao", 0)), Err(ConnectorError::Auth("x".to_string())));
}

#[test]
fn the_echo_text_is_trimmed_and_prefixed() {
    use crate::connectors::runtime::contact::mirror::echo_text;
    assert_eq!(echo_text(&app_message("m", "  ciao\n", 0)).as_deref(), Some("Tu (app): ciao"));
    assert_eq!(echo_text(&app_message("m", "", 0)), None);
}
