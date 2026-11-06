//! Le sotto-chat seguono il ciclo di vita dei thread: eliminazione, workspace rimosso,
//! conclusione (chiusura) e riapertura.

use super::*;
use crate::connectors::testing::SubchatOp;
use crate::hub::NoticeCategory;

fn concluded(message_id: &str) -> ContactNotice {
    ContactNotice { category: NoticeCategory::ThreadConcluded, ..contact_notice(message_id, "Concluso") }
}

/// Le sotto-chat su cui il connettore ha ricevuto l'operazione, in ordine.
fn calls(rig: &Rig, op: SubchatOp) -> Vec<String> {
    rig.contact.lifecycle_calls(op).into_iter().map(|(_, subchat)| subchat).collect()
}

fn thread_removed(rig: &Rig, removed: &ThreadKey) -> Result<(), ConnectorError> {
    tauri::async_runtime::block_on(rig.cc().thread_removed(removed))
}

fn channel_removed(rig: &Rig, channel: &Channel) -> Result<(), ConnectorError> {
    tauri::async_runtime::block_on(rig.cc().channel_removed(channel))
}

fn is_closed(rig: &Rig, subchat: &str) -> bool {
    rig.t.state.lock().subchats.is_closed(&key(subchat))
}

fn close_in_map(rig: &Rig, subchat: &str) {
    rig.t.state.lock().subchats.set_closed(&key(subchat), true);
}

#[test]
fn a_removed_thread_deletes_its_subchat_once_and_the_entry_goes() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    map_subchat(&rig, thread(Channel::General, "thr-2"), "sub-2");
    thread_removed(&rig, &thread(Channel::General, "thr-1")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Delete), vec!["sub-1"]);
    assert_eq!(mapped_thread(&rig, "sub-1"), None);
    assert_eq!(mapped_thread(&rig, "sub-2"), Some(thread(Channel::General, "thr-2")), "gli altri thread restano");
    // Un secondo avviso per lo stesso thread non trova piu' nulla da eliminare.
    thread_removed(&rig, &thread(Channel::General, "thr-1")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Delete), vec!["sub-1"]);
}

#[test]
fn a_removed_workspace_deletes_the_subchats_of_all_its_threads() {
    let rig = group_rig();
    map_subchat(&rig, thread(ws(), "thr-a"), "sub-1");
    map_subchat(&rig, thread(ws(), "thr-b"), "sub-2");
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-3");
    channel_removed(&rig, &ws()).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Delete), vec!["sub-1", "sub-2"]);
    assert_eq!(rig.t.state.lock().subchats.len(), 1);
    assert_eq!(mapped_thread(&rig, "sub-3"), Some(thread(Channel::General, "thr-1")));
}

#[test]
fn a_subchat_already_gone_from_the_service_is_not_an_error() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    rig.contact.delete_remotely("sub-1");
    thread_removed(&rig, &thread(Channel::General, "thr-1")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Delete), vec!["sub-1"]);
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
}

#[test]
fn a_service_error_while_deleting_is_logged_and_the_entry_stays_without_retries() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    *rig.contact.lifecycle_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    thread_removed(&rig, &thread(Channel::General, "thr-1")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Delete), vec!["sub-1"], "un solo tentativo");
    assert_eq!(mapped_thread(&rig, "sub-1"), Some(thread(Channel::General, "thr-1")));
}

#[test]
fn an_error_that_stops_the_supervisor_while_deleting_propagates() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    *rig.contact.lifecycle_error.lock().unwrap() = Some(ConnectorError::Auth("x".to_string()));
    assert_eq!(thread_removed(&rig, &thread(Channel::General, "thr-1")), Err(ConnectorError::Auth("x".to_string())));
}

#[test]
fn a_chat_without_subchats_is_never_called() {
    let rig = Rig::linked();
    // La voce c'e' ma la chat non ha (piu') le sotto-chat: si toglie senza chiamare il servizio.
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    thread_removed(&rig, &thread(Channel::General, "thr-1")).unwrap();
    assert!(rig.contact.lifecycle.lock().unwrap().is_empty());
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
    // Nessuna voce, nessuna chiamata, nemmeno per una conclusione.
    rig.notice(&concluded("msg-1")).unwrap();
    assert!(rig.contact.lifecycle.lock().unwrap().is_empty());
}

#[test]
fn a_concluded_thread_closes_its_subchat_after_the_notice() {
    let rig = group_rig();
    rig.notice(&contact_notice("msg-1", "Risposta")).unwrap();
    rig.notice(&concluded("msg-2")).unwrap();
    assert_eq!(sent(&rig), vec![to(Some("sub-1"), "Risposta"), to(Some("sub-1"), "Concluso")]);
    assert_eq!(calls(&rig, SubchatOp::Close), vec!["sub-1"]);
    assert!(is_closed(&rig, "sub-1"));
    assert_eq!(mapped_thread(&rig, "sub-1"), Some(thread(Channel::General, "thr-1")), "la voce resta");
}

#[test]
fn a_conclusion_closes_the_subchat_even_when_its_notice_is_refused() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    rig.contact.refuse(NoticeCategory::ThreadConcluded);
    rig.notice(&concluded("msg-1")).unwrap();
    assert!(sent(&rig).is_empty());
    assert_eq!(calls(&rig, SubchatOp::Close), vec!["sub-1"]);
}

#[test]
fn closing_twice_or_for_another_thread_does_nothing() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    map_subchat(&rig, thread(Channel::General, "thr-2"), "sub-2");
    rig.contact.refuse(NoticeCategory::ThreadConcluded);
    rig.notice(&concluded("msg-1")).unwrap();
    rig.notice(&concluded("msg-2")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Close), vec!["sub-1"], "gia' chiusa, e sub-2 e' di un altro thread");
    assert!(!is_closed(&rig, "sub-2"));
}

#[test]
fn a_failed_close_is_logged_and_retried_only_by_the_next_conclusion() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    rig.contact.refuse(NoticeCategory::ThreadConcluded);
    *rig.contact.lifecycle_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.notice(&concluded("msg-1")).unwrap();
    assert!(!is_closed(&rig, "sub-1"));
    *rig.contact.lifecycle_error.lock().unwrap() = None;
    rig.notice(&concluded("msg-2")).unwrap();
    assert!(is_closed(&rig, "sub-1"));
    assert_eq!(calls(&rig, SubchatOp::Close), vec!["sub-1", "sub-1"]);
}

#[test]
fn a_subchat_gone_while_closing_is_forgotten() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    rig.contact.refuse(NoticeCategory::ThreadConcluded);
    rig.contact.delete_remotely("sub-1");
    rig.notice(&concluded("msg-1")).unwrap();
    assert_eq!(rig.t.state.lock().subchats.len(), 0);
}

#[test]
fn a_later_notice_reopens_the_closed_subchat_once() {
    let rig = group_rig();
    rig.notice(&concluded("msg-1")).unwrap();
    assert!(is_closed(&rig, "sub-1"));
    rig.notice(&contact_notice("msg-2", "Di nuovo")).unwrap();
    rig.notice(&contact_notice("msg-3", "E ancora")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Reopen), vec!["sub-1"]);
    assert!(!is_closed(&rig, "sub-1"));
    assert_eq!(rig.contact.opened_titles().len(), 1, "la sotto-chat e' sempre la stessa");
    assert_eq!(sent(&rig)[1..], [to(Some("sub-1"), "Di nuovo"), to(Some("sub-1"), "E ancora")]);
}

#[test]
fn a_message_in_a_closed_subchat_reopens_it_and_reaches_the_thread() {
    let rig = group_rig();
    map_subchat(&rig, thread(Channel::General, "thr-1"), "sub-1");
    close_in_map(&rig, "sub-1");
    rig.inbound(in_group("m1", "u1", Some("sub-1"), "ci sono")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Reopen), vec!["sub-1"]);
    assert!(!is_closed(&rig, "sub-1"));
    assert_eq!(written(&rig), vec![(Channel::General, "thr-1".to_string(), None)]);
}

#[test]
fn a_failed_reopen_does_not_block_the_message_and_is_not_retried() {
    let rig = group_rig();
    rig.notice(&concluded("msg-1")).unwrap();
    *rig.contact.lifecycle_error.lock().unwrap() = Some(ConnectorError::Transient("x".to_string()));
    rig.notice(&contact_notice("msg-2", "Di nuovo")).unwrap();
    rig.notice(&contact_notice("msg-3", "E ancora")).unwrap();
    assert_eq!(calls(&rig, SubchatOp::Reopen), vec!["sub-1"], "un solo tentativo");
    assert_eq!(sent(&rig).len(), 3);
}

#[test]
fn a_subchat_gone_while_reopening_is_recreated() {
    let rig = group_rig();
    rig.notice(&concluded("msg-1")).unwrap();
    rig.contact.delete_remotely("sub-1");
    rig.notice(&contact_notice("msg-2", "Di nuovo")).unwrap();
    assert_eq!(rig.contact.opened_titles().len(), 2);
    assert_eq!(sent(&rig).last(), Some(&to(Some("sub-2"), "Di nuovo")));
    assert_eq!(mapped_thread(&rig, "sub-1"), None);
}
