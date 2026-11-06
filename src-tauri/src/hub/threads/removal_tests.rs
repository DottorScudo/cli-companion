//! Eliminazione di un thread: cosa si toglie, cosa resta e le scritture
//! successive sul thread sparito (un lavoro del motore gia' partito).

use super::*;
use crate::hub::model::HubEventKind;

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

/// Due thread: `t1` con un task e una proposta collegati e un messaggio, `t2` vuoto.
fn two_threads() -> ChannelThreads {
    let mut list = ChannelThreads::default();
    list.create_thread(&ws(), "t1".to_string(), "Obiettivo", 1).unwrap();
    list.create_thread(&ws(), "t2".to_string(), "Altro", 2).unwrap();
    list.link_tasks("t1", &["task-1".to_string()], 3).unwrap();
    list.link_proposal("t1", "p-1", 3).unwrap();
    list.add_message("t1", text("m1"), 4).unwrap();
    list
}

fn text(id: &str) -> NewMessage {
    NewMessage {
        id: id.to_string(),
        author: Author::User,
        body: "ciao".to_string(),
        reply_to: None,
        event: None,
        origin: None,
    }
}

#[test]
fn removing_a_thread_returns_it_and_leaves_the_others_untouched() {
    let mut list = two_threads();
    let other = list.thread("t2").cloned().unwrap();
    let removed = list.remove_thread("t1").unwrap();
    assert_eq!((removed.id.as_str(), removed.messages.len()), ("t1", 1));
    assert!(list.thread("t1").is_none());
    assert_eq!(list.threads, [other]);
    // Task e proposta non sono piu' di nessun thread: un loro evento ne apre uno nuovo.
    assert!(list.thread_of_task("task-1").is_none() && list.thread_of_proposal("p-1").is_none());
}

#[test]
fn a_closed_thread_can_be_removed_too() {
    let mut list = two_threads();
    list.set_state("t1", ThreadState::Closed, 5).unwrap();
    assert!(list.remove_thread("t1").is_ok());
    assert_eq!(list.threads.len(), 1);
}

#[test]
fn removing_a_missing_thread_changes_nothing() {
    let mut list = two_threads();
    let before = list.clone();
    assert_eq!(list.remove_thread("nope").unwrap_err(), THREAD_NOT_FOUND);
    list.remove_thread("t1").unwrap();
    assert_eq!(list.remove_thread("t1").unwrap_err(), THREAD_NOT_FOUND, "gia' tolto");
    assert_eq!(list.threads.len(), before.threads.len() - 1);
}

#[test]
fn writes_on_a_removed_thread_fail_without_recreating_it() {
    let mut list = two_threads();
    list.remove_thread("t1").unwrap();
    let after = list.clone();
    let event = NewMessage {
        event: Some(HubEvent::of_task(HubEventKind::TaskDone, "task-1")),
        author: Author::System,
        ..text("e1")
    };
    assert_eq!(list.add_message("t1", text("m2"), 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.add_message("t1", event, 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.set_summary("t1", "riassunto".to_string()).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.set_state("t1", ThreadState::Closed, 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.set_owner("t1", "mario", 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.link_tasks("t1", &["task-2".to_string()], 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list.link_proposal("t1", "p-2", 6).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list, after);
}
