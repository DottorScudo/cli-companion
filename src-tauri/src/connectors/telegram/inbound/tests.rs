use super::*;
use crate::connectors::contracts::{ContactCommand, RemoteFile};

fn message(message_id: i64, text: Option<&str>) -> ChatMessage {
    ChatMessage {
        chat_id: 42,
        message_id,
        sender_id: 7,
        sender_name: "Mario".to_string(),
        text: text.map(str::to_string),
        caption: None,
        reply_to: None,
        media_group: None,
        files: Vec::new(),
    }
}

fn photo(message_id: i64, group: &str, caption: Option<&str>) -> ChatMessage {
    ChatMessage {
        caption: caption.map(str::to_string),
        media_group: Some(group.to_string()),
        files: vec![RemoteFile { id: format!("f{message_id}"), name: format!("foto-{message_id}.jpg"), size: None }],
        ..message(message_id, None)
    }
}

fn update(id: i64, kind: UpdateKind) -> Update {
    Update { id, kind }
}

#[test]
fn a_text_message_maps_every_field() {
    let mut original = message(10, Some("ciao"));
    original.reply_to = Some(3);
    let inbound = to_inbound(original);
    assert_eq!(inbound.external_id, "42:10");
    assert_eq!((inbound.account.as_str(), inbound.chat.as_str()), ("7", "42"));
    assert_eq!(inbound.sender_name, "Mario");
    assert_eq!(inbound.text, "ciao");
    assert_eq!(inbound.reply_to_external.as_deref(), Some("42:3"));
    assert!(inbound.files.is_empty() && inbound.pairing_code.is_none() && inbound.command.is_none());
}

#[test]
fn the_caption_is_the_text_when_there_is_no_text() {
    let mut original = message(1, None);
    original.caption = Some("didascalia".to_string());
    assert_eq!(to_inbound(original).text, "didascalia");
    assert_eq!(to_inbound(message(1, None)).text, "");
}

#[test]
fn a_start_with_a_code_is_a_pairing_invite_that_keeps_its_text() {
    let inbound = to_inbound(message(1, Some("/start CODICE-1")));
    assert_eq!(inbound.pairing_code.as_deref(), Some("CODICE-1"));
    assert_eq!(inbound.command, None);
    assert_eq!(inbound.text, "/start CODICE-1");
}

#[test]
fn a_command_keeps_its_text() {
    let inbound = to_inbound(message(1, Some("/use@MuseBot 2")));
    assert_eq!(inbound.command, Some(ContactCommand::Use("2".to_string())));
    assert_eq!(inbound.text, "/use@MuseBot 2");
    assert_eq!(to_inbound(message(2, Some("/boh"))).command, Some(ContactCommand::Unknown));
}

#[test]
fn a_caption_is_never_a_command() {
    let mut original = message(1, None);
    original.caption = Some("/unlink".to_string());
    let inbound = to_inbound(original);
    assert_eq!((inbound.command, inbound.pairing_code), (None, None));
}

#[test]
fn consecutive_album_parts_merge_into_one_message() {
    let merged = merge_albums(vec![photo(1, "g", None), photo(2, "g", Some("viaggio")), photo(3, "g", Some("altra"))]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].message_id, 1);
    assert_eq!(merged[0].caption.as_deref(), Some("viaggio"));
    assert_eq!(merged[0].files.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["f1", "f2", "f3"]);
}

#[test]
fn different_groups_chats_or_plain_messages_do_not_merge() {
    let mut other_chat = photo(2, "g", None);
    other_chat.chat_id = 43;
    let merged = merge_albums(vec![
        photo(1, "g", None),
        other_chat,
        photo(3, "h", None),
        message(4, Some("x")),
        message(5, Some("y")),
    ]);
    assert_eq!(merged.iter().map(|m| m.message_id).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn an_interleaved_message_splits_an_album() {
    let merged = merge_albums(vec![photo(1, "g", None), message(2, Some("x")), photo(3, "g", None)]);
    assert_eq!(merged.len(), 3);
}

#[test]
fn only_messages_are_built_from_a_batch() {
    let updates = vec![
        update(1, UpdateKind::Ignored("sticker")),
        update(2, UpdateKind::Message(message(10, Some("uno")))),
        update(3, UpdateKind::JoinedChat(-5)),
        update(4, UpdateKind::Message(message(11, Some("due")))),
    ];
    let built = build_messages(&updates);
    assert_eq!(built.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), vec!["uno", "due"]);
    assert!(build_messages(&[]).is_empty());
}

#[test]
fn an_album_in_a_batch_becomes_one_inbound_message_with_all_files() {
    let updates = vec![
        update(1, UpdateKind::Message(photo(1, "g", Some("tre foto")))),
        update(2, UpdateKind::Message(photo(2, "g", None))),
        update(3, UpdateKind::Message(photo(3, "g", None))),
    ];
    let built = build_messages(&updates);
    assert_eq!(built.len(), 1);
    assert_eq!((built[0].external_id.as_str(), built[0].text.as_str()), ("42:1", "tre foto"));
    assert_eq!(built[0].files.len(), 3);
}
