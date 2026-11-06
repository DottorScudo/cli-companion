use super::*;
use serde_json::json;

fn private_message(extra: Value) -> Value {
    let mut message = json!({
        "message_id": 10,
        "from": {"id": 42, "is_bot": false, "first_name": "Mario", "last_name": "Rossi"},
        "chat": {"id": 42, "type": "private"},
        "date": 1700000000
    });
    if let (Some(base), Some(more)) = (message.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    message
}

fn one(raw: Value) -> Update {
    let updates = parse_updates(&json!([raw]));
    assert_eq!(updates.len(), 1, "{updates:?}");
    updates.into_iter().next().unwrap()
}

fn message_of(update: Update) -> ChatMessage {
    match update.kind {
        UpdateKind::Message(message) => message,
        other => panic!("atteso un messaggio, trovato {other:?}"),
    }
}

#[test]
fn a_private_text_message_is_parsed() {
    let update = one(json!({"update_id": 100, "message": private_message(json!({"text": "ciao"}))}));
    assert_eq!(update.id, 100);
    let message = message_of(update);
    assert_eq!((message.chat_id, message.message_id, message.sender_id), (42, 10, 42));
    assert_eq!(message.sender_name, "Mario Rossi");
    assert_eq!(message.text.as_deref(), Some("ciao"));
    assert!(message.caption.is_none() && message.reply_to.is_none() && message.files.is_empty());
}

#[test]
fn a_reply_keeps_the_original_message_id() {
    let raw = private_message(json!({"text": "si", "reply_to_message": {"message_id": 7, "text": "vuoi?"}}));
    assert_eq!(message_of(one(json!({"update_id": 1, "message": raw}))).reply_to, Some(7));
}

#[test]
fn a_photo_with_a_caption_has_the_caption_and_the_largest_size() {
    let raw = private_message(json!({
        "caption": "guarda",
        "photo": [{"file_id": "s", "file_size": 10, "width": 10, "height": 10}, {"file_id": "l", "file_size": 99, "width": 90, "height": 90}]
    }));
    let message = message_of(one(json!({"update_id": 1, "message": raw})));
    assert_eq!(message.caption.as_deref(), Some("guarda"));
    assert!(message.text.is_none());
    assert_eq!(message.files.len(), 1);
    assert_eq!(message.files[0].id, "l");
}

#[test]
fn a_document_without_a_caption_is_still_a_message() {
    let raw = private_message(json!({"document": {"file_id": "d", "file_name": "a.txt", "file_size": 5}}));
    let message = message_of(one(json!({"update_id": 1, "message": raw})));
    assert_eq!(message.files[0].name, "a.txt");
    assert!(message.text.is_none() && message.caption.is_none());
}

#[test]
fn album_parts_carry_the_media_group() {
    let raw = private_message(json!({"media_group_id": "9", "photo": [{"file_id": "p"}]}));
    assert_eq!(message_of(one(json!({"update_id": 1, "message": raw}))).media_group.as_deref(), Some("9"));
}

#[test]
fn unsupported_content_is_ignored_with_its_kind() {
    for (key, content) in [
        ("sticker", json!({"file_id": "s"})),
        ("voice", json!({"file_id": "v"})),
        ("video", json!({"file_id": "v"})),
        ("location", json!({"latitude": 1.0, "longitude": 2.0})),
        ("poll", json!({"id": "p"})),
    ] {
        let raw = private_message(json!({ key: content }));
        let update = one(json!({"update_id": 5, "message": raw}));
        assert_eq!(update.kind, UpdateKind::Ignored(key), "{key}");
    }
    let empty = one(json!({"update_id": 5, "message": private_message(json!({}))}));
    assert_eq!(empty.kind, UpdateKind::Ignored("other"));
    let blank = one(json!({"update_id": 5, "message": private_message(json!({"text": "  "}))}));
    assert_eq!(blank.kind, UpdateKind::Ignored("other"));
}

#[test]
fn messages_of_groups_without_topics_and_of_channels_are_ignored() {
    for chat in [
        json!({"id": -500, "type": "group"}),
        json!({"id": -100500, "type": "supergroup"}),
        json!({"id": -100500, "type": "supergroup", "is_forum": false}),
        json!({"id": -100500, "type": "supergroup", "is_forum": "true"}),
        json!({"id": -100500, "type": "channel"}),
        json!({"id": -100500, "type": "group", "is_forum": true}),
    ] {
        let mut raw = private_message(json!({"text": "ciao"}));
        raw["chat"] = chat.clone();
        let update = one(json!({"update_id": 1, "message": raw}));
        assert_eq!(update.kind, UpdateKind::Ignored("unsupported_chat"), "{chat}");
    }
}

fn forum_message(extra: Value) -> Value {
    let mut raw = private_message(extra);
    raw["chat"] = json!({"id": FORUM, "type": "supergroup", "is_forum": true, "title": "HUB"});
    raw
}

/// Un messaggio scritto nel topic `TOPIC`, come lo manda Telegram.
fn topic_message(extra: Value) -> Value {
    let mut raw = forum_message(extra);
    raw["message_thread_id"] = json!(TOPIC);
    raw["is_topic_message"] = json!(true);
    if raw.get("reply_to_message").is_none() {
        raw["reply_to_message"] = json!({
            "message_id": TOPIC,
            "forum_topic_created": {"name": "Generale › Piano", "icon_color": 7322096},
            "chat": {"id": FORUM, "type": "supergroup", "is_forum": true}
        });
    }
    raw
}

const FORUM: i64 = -1_001_234_567_890;
const TOPIC: i64 = 55;

#[test]
fn a_topic_message_carries_its_topic() {
    let message = message_of(one(json!({"update_id": 1, "message": topic_message(json!({"text": "ciao"}))})));
    assert_eq!((message.chat_id, message.topic_id, message.message_id), (FORUM, Some(TOPIC), 10));
    assert_eq!(message.text.as_deref(), Some("ciao"));
}

#[test]
fn a_topic_message_that_is_not_a_reply_has_no_reply_to() {
    // Telegram lega ogni messaggio di un topic al messaggio di servizio che lo ha aperto.
    let message = message_of(one(json!({"update_id": 1, "message": topic_message(json!({"text": "ciao"}))})));
    assert_eq!(message.reply_to, None);
}

#[test]
fn a_real_reply_inside_a_topic_keeps_its_target() {
    let raw = topic_message(json!({"text": "si", "reply_to_message": {"message_id": 70, "text": "vuoi?"}}));
    let message = message_of(one(json!({"update_id": 1, "message": raw})));
    assert_eq!((message.topic_id, message.reply_to), (Some(TOPIC), Some(70)));
}

#[test]
fn a_message_in_the_general_topic_has_no_topic() {
    let message = message_of(one(json!({"update_id": 1, "message": forum_message(json!({"text": "ciao"}))})));
    assert_eq!((message.chat_id, message.topic_id), (FORUM, None));
    // Un id del topic senza `is_topic_message` (es. una risposta in «General») non conta.
    let mut raw = forum_message(json!({"text": "x", "reply_to_message": {"message_id": 3}}));
    raw["message_thread_id"] = json!(3);
    let message = message_of(one(json!({"update_id": 1, "message": raw})));
    assert_eq!((message.topic_id, message.reply_to), (None, Some(3)));
}

#[test]
fn a_private_chat_never_has_a_topic() {
    let raw = private_message(json!({"text": "x", "message_thread_id": 9, "is_topic_message": true}));
    assert_eq!(message_of(one(json!({"update_id": 1, "message": raw}))).topic_id, None);
}

#[test]
fn malformed_topic_ids_are_dropped() {
    for id in [json!(0), json!(-4), json!("55"), json!(null), json!(1.5)] {
        let mut raw = topic_message(json!({"text": "x"}));
        raw["message_thread_id"] = id.clone();
        assert_eq!(message_of(one(json!({"update_id": 1, "message": raw}))).topic_id, None, "{id}");
    }
}

#[test]
fn topic_service_messages_are_ignored() {
    for (key, content) in [
        ("forum_topic_created", json!({"name": "x", "icon_color": 1})),
        ("forum_topic_edited", json!({"name": "y"})),
        ("forum_topic_closed", json!({})),
        ("forum_topic_reopened", json!({})),
        ("general_forum_topic_hidden", json!({})),
        ("general_forum_topic_unhidden", json!({})),
    ] {
        let mut raw = topic_message(json!({ key: content }));
        raw.as_object_mut().unwrap().remove("reply_to_message");
        assert_eq!(one(json!({"update_id": 1, "message": raw})).kind, UpdateKind::Ignored("topic_service"), "{key}");
    }
}

#[test]
fn messages_from_bots_or_without_sender_are_ignored() {
    let mut bot = private_message(json!({"text": "x"}));
    bot["from"]["is_bot"] = json!(true);
    assert_eq!(one(json!({"update_id": 1, "message": bot})).kind, UpdateKind::Ignored("bot_sender"));
    let mut anonymous = private_message(json!({"text": "x"}));
    anonymous.as_object_mut().unwrap().remove("from");
    assert_eq!(one(json!({"update_id": 1, "message": anonymous})).kind, UpdateKind::Ignored("sender_missing"));
}

fn member_change(chat_type: &str, status: &str) -> Value {
    json!({"update_id": 3, "my_chat_member": {
        "chat": {"id": -77, "type": chat_type},
        "from": {"id": 42},
        "new_chat_member": {"status": status, "user": {"id": 1}}
    }})
}

#[test]
fn being_added_to_a_group_without_topics_is_a_reason_to_leave() {
    for status in ["member", "administrator"] {
        assert_eq!(one(member_change("group", status)).kind, UpdateKind::JoinedChat(-77), "{status}");
        assert_eq!(one(member_change("supergroup", status)).kind, UpdateKind::JoinedChat(-77));
        assert_eq!(one(member_change("channel", status)).kind, UpdateKind::JoinedChat(-77));
    }
}

#[test]
fn being_added_to_a_forum_waits_for_the_pairing() {
    for status in ["member", "administrator"] {
        let mut change = member_change("supergroup", status);
        change["my_chat_member"]["chat"]["is_forum"] = json!(true);
        assert_eq!(one(change).kind, UpdateKind::Ignored("joined_forum"), "{status}");
    }
}

#[test]
fn other_membership_changes_are_ignored() {
    for status in ["left", "kicked", "restricted", "creator"] {
        assert_eq!(one(member_change("group", status)).kind, UpdateKind::Ignored("chat_member"), "{status}");
    }
    // Un utente che blocca il bot in privato non e' un'uscita da fare.
    assert_eq!(one(member_change("private", "member")).kind, UpdateKind::Ignored("chat_member"));
}

#[test]
fn unknown_update_kinds_are_ignored_but_counted_for_the_cursor() {
    let edited = one(json!({"update_id": 8, "edited_message": private_message(json!({"text": "x"}))}));
    assert_eq!((edited.id, edited.kind), (8, UpdateKind::Ignored("unsupported_update")));
    let nothing = one(json!({"update_id": 9}));
    assert_eq!(nothing.kind, UpdateKind::Ignored("unsupported_update"));
}

#[test]
fn updates_without_a_valid_id_are_dropped() {
    let updates = parse_updates(&json!([
        {"message": private_message(json!({"text": "x"}))},
        {"update_id": "1"},
        {"update_id": null},
        {"update_id": 1.5},
        {"update_id": 18446744073709551615u64},
        {"update_id": 2, "message": private_message(json!({"text": "ok"}))}
    ]));
    assert_eq!(updates.iter().map(|u| u.id).collect::<Vec<_>>(), vec![2]);
}

#[test]
fn the_next_offset_is_the_highest_id_plus_one() {
    let updates = parse_updates(&json!([{"update_id": 5}, {"update_id": 9}, {"update_id": 7}]));
    assert_eq!(next_offset(&updates), Some(10));
    assert_eq!(next_offset(&[]), None);
    let max = parse_updates(&json!([{"update_id": i64::MAX}]));
    assert_eq!(next_offset(&max), Some(i64::MAX));
}

#[test]
fn hostile_json_never_panics() {
    let mut nested = json!(1);
    for _ in 0..50 {
        nested = json!({"a": [nested]});
    }
    let hostile_values = [
        json!(null),
        json!(5),
        json!("testo"),
        json!({}),
        json!([null, 5, "x", [], {}, [[]]]),
        json!({"update_id": 1, "message": null}),
        json!({"update_id": 1, "message": []}),
        json!({"update_id": 1, "message": "testo"}),
        json!({"update_id": 1, "message": {"chat": "x", "from": 5}}),
        json!({"update_id": 1, "message": {"chat": {"id": "1", "type": "private"}}}),
        json!({"update_id": 1, "message": {"chat": {"id": 1}, "from": {"id": 1}}}),
        json!({"update_id": 1, "message": {"chat": {"id": 1e300, "type": "private"}, "from": {"id": 1}, "message_id": 1}}),
        json!({"update_id": 1, "message": {"chat": {"id": 18446744073709551615u64, "type": "private"}, "from": {"id": 1}, "message_id": 1, "text": "x"}}),
        json!({"update_id": 1, "message": {"chat": {"id": 1, "type": "private"}, "from": {"id": 1, "first_name": ["x"]}, "message_id": "1", "text": "x"}}),
        json!({"update_id": 1, "message": {"chat": {"id": 1, "type": "private"}, "from": {"id": 1, "first_name": "\u{0}\u{202e}😀"}, "message_id": 1, "text": 5, "caption": [], "photo": {"a": 1}, "document": 3, "reply_to_message": "x", "media_group_id": 7}}),
        json!({"update_id": 1, "message": nested}),
        json!({"update_id": 1, "my_chat_member": null}),
        json!({"update_id": 1, "my_chat_member": {"chat": {"id": 1, "type": "group"}, "new_chat_member": "x"}}),
        json!({"update_id": 1, "my_chat_member": {"chat": [], "new_chat_member": {"status": 5}}}),
    ];
    for value in hostile_values {
        let _ = parse_updates(&value);
        let _ = parse_updates(&json!([value]));
    }
    let message = json!({"update_id": 1, "message": {"chat": {"id": 1, "type": "private"}, "from": {"id": 1}, "message_id": 1, "text": 5}});
    assert_eq!(one(message).kind, UpdateKind::Ignored("other"));
}

#[test]
fn a_result_that_is_not_an_array_is_empty() {
    for value in [json!(null), json!({}), json!("x"), json!(3)] {
        assert!(parse_updates(&value).is_empty());
    }
}
