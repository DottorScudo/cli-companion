//! Autori `external` e origine dei messaggi: validazione e compatibilita' del formato.

use super::*;
use crate::hub::text::{EXTERNAL_ID_MAX, EXTERNAL_NAME_MAX};

fn ws() -> Channel {
    Channel::Workspace("ws-1".to_string())
}

fn with_thread() -> ChannelThreads {
    let mut list = ChannelThreads::default();
    list.create_thread(&ws(), "t1".to_string(), "Posta", 0).unwrap();
    list
}

fn external_author(name: &str) -> Author {
    Author::External { connector: "email".to_string(), instance: "casa-1".to_string(), name: name.to_string() }
}

fn origin(external_id: &str) -> MessageOrigin {
    MessageOrigin {
        connector: "email".to_string(),
        instance: "casa-1".to_string(),
        external_id: external_id.to_string(),
    }
}

fn external(id: &str, name: &str, origin: Option<MessageOrigin>) -> NewMessage {
    NewMessage {
        id: id.to_string(),
        author: external_author(name),
        body: "Ciao dal mondo".to_string(),
        reply_to: None,
        event: None,
        origin,
    }
}

fn attachment() -> Attachment {
    Attachment { id: "att-1-1".to_string(), name: "a.txt".to_string(), mime: "text/plain".to_string(), size: 3 }
}

#[test]
fn an_external_message_is_stored_with_a_cleaned_author_and_its_origin() {
    let mut list = with_thread();
    let new = external("m1", "  Mario\nRossi ", Some(origin("<abc@mail>")));
    let m = list.add_message("t1", new, 5).unwrap().clone();
    assert_eq!(m.author, external_author("Mario Rossi"));
    assert_eq!(m.origin, Some(origin("<abc@mail>")));
    assert_eq!(m.kind, MessageKind::Text);
}

#[test]
fn an_external_author_requires_a_matching_origin() {
    let mut list = with_thread();
    let before = list.clone();
    let other_instance = MessageOrigin { instance: "altra".to_string(), ..origin("x") };
    let other_connector = MessageOrigin { connector: "chat".to_string(), ..origin("x") };
    for bad in [None, Some(other_instance), Some(other_connector)] {
        assert!(list.add_message("t1", external("m1", "Mario", bad), 1).is_err());
    }
    assert_eq!(list, before);
}

#[test]
fn external_authors_are_validated() {
    let mut list = with_thread();
    assert!(list.add_message("t1", external("m1", " \n ", Some(origin("x"))), 1).is_err());
    let long = list.add_message("t1", external("m2", &"é".repeat(300), Some(origin("x"))), 1).unwrap();
    assert!(matches!(&long.author, Author::External { name, .. } if name.chars().count() <= EXTERNAL_NAME_MAX + 1));
    for bad in ["", "a/b", "a b", &"x".repeat(65)] {
        let author = Author::External { connector: bad.to_string(), instance: "i".to_string(), name: "M".to_string() };
        let new = NewMessage { author, ..external("m3", "M", Some(origin("x"))) };
        assert!(list.add_message("t1", new, 1).is_err(), "connector {bad:?}");
        let author =
            Author::External { connector: "email".to_string(), instance: bad.to_string(), name: "M".to_string() };
        let origin = MessageOrigin { instance: bad.to_string(), ..origin("x") };
        let new = NewMessage { author, origin: Some(origin), ..external("m3", "M", None) };
        assert!(list.add_message("t1", new, 1).is_err(), "instance {bad:?}");
    }
}

#[test]
fn the_origin_id_is_bounded_and_clean() {
    let mut list = with_thread();
    assert!(list.add_message("t1", external("m1", "M", Some(origin(&"x".repeat(EXTERNAL_ID_MAX)))), 1).is_ok());
    for bad in ["", "a\nb", "a\u{0}b", &"x".repeat(EXTERNAL_ID_MAX + 1)] {
        assert!(list.add_message("t1", external("m2", "M", Some(origin(bad))), 1).is_err(), "{bad:?}");
    }
    assert_eq!(list.thread("t1").unwrap().messages.len(), 1);
}

#[test]
fn a_user_message_may_carry_an_origin_but_it_is_validated() {
    let mut list = with_thread();
    let user = |id: &str, o: Option<MessageOrigin>| NewMessage { author: Author::User, ..external(id, "x", o) };
    assert_eq!(list.add_message("t1", user("m1", Some(origin("42"))), 1).unwrap().origin, Some(origin("42")));
    assert!(list.add_message("t1", user("m2", Some(origin(""))), 1).is_err());
    assert_eq!(list.add_message("t1", user("m3", None), 1).unwrap().origin, None);
}

#[test]
fn external_authors_may_attach_files_but_bots_and_system_may_not() {
    let mut list = with_thread();
    let mut with_files =
        |new: NewMessage| list.add_message_with_attachments("t1", new, vec![attachment()], 1).map(|m| m.id.clone());
    let empty_text = NewMessage { body: String::new(), ..external("m1", "M", Some(origin("1"))) };
    assert_eq!(with_files(empty_text).unwrap(), "m1");
    let bot = NewMessage { author: Author::Bot { name: "ada".to_string() }, ..external("m2", "M", None) };
    assert!(with_files(bot).is_err());
    let system = NewMessage { author: Author::System, ..external("m3", "M", None) };
    assert!(with_files(system).is_err());
}

#[test]
fn messages_written_before_origins_and_external_authors_still_load() {
    let json = r#"{"threads":[{"id":"t1","channel":"ws-1","title":"Vecchio","state":"open","created_at":1,"updated_at":2,
        "messages":[
          {"id":"m1","author":{"type":"user"},"kind":"text","body":"a","created_at":1},
          {"id":"m2","author":{"type":"bot","name":"ada"},"kind":"text","body":"b","created_at":2},
          {"id":"m3","author":{"type":"system"},"kind":"text","body":"c","created_at":2}]}]}"#;
    let list: ChannelThreads = serde_json::from_str(json).unwrap();
    let messages = &list.thread("t1").unwrap().messages;
    assert_eq!(messages.len(), 3);
    assert!(messages.iter().all(|m| m.origin.is_none()));
}

#[test]
fn external_authors_and_origins_roundtrip_in_json() {
    let mut list = with_thread();
    list.add_message("t1", external("m1", "Mario", Some(origin("<id@x>"))), 1).unwrap();
    let json = serde_json::to_string(&list).unwrap();
    assert!(
        json.contains(r#""author":{"type":"external","connector":"email","instance":"casa-1","name":"Mario"}"#),
        "{json}"
    );
    assert!(json.contains(r#""origin":{"connector":"email","instance":"casa-1","external_id":"<id@x>"}"#), "{json}");
    assert_eq!(serde_json::from_str::<ChannelThreads>(&json).unwrap(), list);
}
