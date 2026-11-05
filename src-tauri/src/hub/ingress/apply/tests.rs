use super::*;
use crate::hub::model::MessageKind;
use crate::hub::text::{BODY_MAX, TRUNCATED_SUFFIX};

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn author() -> Author {
    Author::External { connector: "email".into(), instance: "casa".into(), name: "Mario".into() }
}

fn origin(external_id: &str) -> MessageOrigin {
    MessageOrigin { connector: "email".into(), instance: "casa".into(), external_id: external_id.into() }
}

fn list_with(thread_id: &str) -> ChannelThreads {
    let mut list = ChannelThreads::default();
    list.create_thread(&ws(), thread_id.to_string(), "Posta", 1).unwrap();
    list
}

fn write<'a>(channel: &'a Channel, thread_id: &'a str, external_id: &str, wake: &'a ExternalWake) -> Write<'a> {
    Write {
        channel,
        thread_id,
        new_title: None,
        author: author(),
        origin: origin(external_id),
        body: "Ciao".to_string(),
        attachments: Vec::new(),
        message_id: format!("msg-{external_id}"),
        wake,
        now: 10,
    }
}

fn stored(applied: Applied) -> Stored {
    match applied {
        Applied::Stored(stored) => stored,
        other => panic!("atteso un messaggio salvato, trovato {other:?}"),
    }
}

fn is_stored(applied: Result<Applied, String>) -> bool {
    matches!(applied, Ok(Applied::Stored(_)))
}

#[test]
fn a_delivery_is_stored_with_author_origin_and_no_reply() {
    let mut list = list_with("thr-1");
    let channel = ws();
    let s = stored(apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::None)).unwrap());
    assert_eq!((s.summary.id.as_str(), s.summary.message_count, s.body.as_str()), ("thr-1", 1, "Ciao"));
    let m = &list.thread("thr-1").unwrap().messages[0];
    assert_eq!((m.author.clone(), m.kind, m.origin.clone()), (author(), MessageKind::Text, Some(origin("a"))));
    assert!(m.reply_to.is_none() && m.event.is_none());
    assert_eq!(list.thread("thr-1").unwrap().updated_at, 10);
}

#[test]
fn the_same_element_is_a_duplicate_and_writes_nothing() {
    let mut list = list_with("thr-1");
    let channel = ws();
    apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::None)).unwrap();
    let before = list.clone();
    let applied = apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::Owner)).unwrap();
    assert_eq!(applied, Applied::Duplicate { message_id: "msg-a".to_string() });
    assert!(!applied.changed());
    assert_eq!(list, before);
}

#[test]
fn the_dedupe_key_is_instance_and_external_id_within_the_thread() {
    let mut list = list_with("thr-1");
    list.create_thread(&ws(), "thr-2".to_string(), "Altro", 1).unwrap();
    let channel = ws();
    apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::None)).unwrap();
    // Un altro id esterno e lo stesso id in un altro thread non sono duplicati.
    assert!(is_stored(apply(&mut list, write(&channel, "thr-1", "b", &ExternalWake::None))));
    assert!(is_stored(apply(&mut list, write(&channel, "thr-2", "a", &ExternalWake::None))));
    // Un'altra istanza con lo stesso id esterno nemmeno.
    let mut other = write(&channel, "thr-1", "a", &ExternalWake::None);
    other.origin.instance = "altra".to_string();
    other.author = Author::External { connector: "email".into(), instance: "altra".into(), name: "M".into() };
    other.message_id = "msg-altra".to_string();
    assert!(is_stored(apply(&mut list, other)));
}

#[test]
fn a_closed_thread_is_reopened() {
    let mut list = list_with("thr-1");
    list.set_state("thr-1", ThreadState::Closed, 2).unwrap();
    let channel = ws();
    let s = stored(apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::None)).unwrap());
    assert_eq!(s.summary.state, ThreadState::Open);
    assert_eq!(list.thread("thr-1").unwrap().state, ThreadState::Open);
}

#[test]
fn a_new_thread_is_created_with_the_title_and_the_message() {
    let mut list = ChannelThreads::default();
    let channel = ws();
    let mut w = write(&channel, "thr-9", "a", &ExternalWake::None);
    w.new_title = Some("Oggetto della mail");
    let s = stored(apply(&mut list, w).unwrap());
    assert_eq!((s.summary.title.as_str(), s.summary.message_count), ("Oggetto della mail", 1));
    assert_eq!(list.thread("thr-9").unwrap().state, ThreadState::Open);
}

#[test]
fn an_unknown_thread_is_an_error_and_changes_nothing() {
    let mut list = list_with("thr-1");
    let before = list.clone();
    let channel = ws();
    assert_eq!(apply(&mut list, write(&channel, "thr-x", "a", &ExternalWake::None)).unwrap_err(), THREAD_NOT_FOUND);
    assert_eq!(list, before);
    assert_eq!(precheck(&list, "thr-x", &origin("a")).unwrap_err(), THREAD_NOT_FOUND);
}

#[test]
fn an_invalid_message_is_rejected() {
    let mut list = list_with("thr-1");
    let channel = ws();
    let mut empty = write(&channel, "thr-1", "a", &ExternalWake::None);
    empty.body = String::new();
    assert!(apply(&mut list, empty).is_err());
    let mut wrong_author = write(&channel, "thr-1", "b", &ExternalWake::None);
    wrong_author.author = Author::User;
    wrong_author.origin = origin("");
    assert!(apply(&mut list, wrong_author).is_err());
}

#[test]
fn the_destination_is_the_given_thread_or_a_new_one_with_a_clean_title() {
    let existing = DeliveryTarget::Existing("thr-1".to_string());
    assert_eq!(destination(&existing, "thr-new"), ("thr-1".to_string(), None));
    let new = |title: &str| destination(&DeliveryTarget::New { title: title.to_string() }, "thr-new");
    assert_eq!(new("  Re:\nciao\u{1b} "), ("thr-new".to_string(), Some("Re: ciao".to_string())));
    assert_eq!(new(" \n "), ("thr-new".to_string(), Some(UNTITLED.to_string())));
    let long = new(&"é".repeat(500)).1.unwrap();
    assert!(crate::hub::text::title(&long).is_ok(), "{}", long.chars().count());
}

#[test]
fn a_long_title_makes_a_valid_thread() {
    let mut list = ChannelThreads::default();
    let channel = ws();
    let (id, title) = destination(&DeliveryTarget::New { title: "x".repeat(500) }, "thr-9");
    let mut w = write(&channel, &id, "a", &ExternalWake::None);
    w.new_title = title.as_deref();
    assert!(apply(&mut list, w).is_ok());
}

#[test]
fn the_wake_target_follows_the_policy() {
    let mut list = list_with("thr-1");
    let channel = ws();
    let thread = |list: &ChannelThreads| list.thread("thr-1").unwrap().clone();
    assert_eq!(wake_target(&thread(&list), &ExternalWake::None), None);
    // Owner senza titolare: nessuno.
    assert_eq!(wake_target(&thread(&list), &ExternalWake::Owner), None);
    let s = stored(apply(&mut list, write(&channel, "thr-1", "a", &ExternalWake::Owner)).unwrap());
    assert_eq!(s.secretary, None);
    list.set_owner("thr-1", "ada", 3).unwrap();
    assert_eq!(wake_target(&thread(&list), &ExternalWake::Owner), Some("ada".to_string()));
    let named = ExternalWake::Secretary("grace".to_string());
    let s = stored(apply(&mut list, write(&channel, "thr-1", "b", &named)).unwrap());
    assert_eq!(s.secretary, Some("grace".to_string()));
}

#[test]
fn the_composed_body_fits_the_message_limits() {
    let mut list = list_with("thr-1");
    let channel = ws();
    let mut w = write(&channel, "thr-1", "a", &ExternalWake::None);
    let note = "Allegati non importati: a.pdf (troppo grande)";
    w.body = super::super::body::compose(&"x".repeat(BODY_MAX * 2), Some(note), false).unwrap();
    let s = stored(apply(&mut list, w).unwrap());
    assert_eq!(s.body.chars().count(), BODY_MAX);
    assert!(s.body.contains(TRUNCATED_SUFFIX));
}
