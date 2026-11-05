use super::*;

fn sample(channel: Channel) -> MessageRef {
    MessageRef { channel, thread_id: "thr-1-2".to_string(), message_id: "msg-3-4".to_string() }
}

#[test]
fn a_reference_round_trips_for_both_kinds_of_channel() {
    for channel in [Channel::General, Channel::Workspace("ws-17-1".to_string())] {
        let original = sample(channel);
        assert_eq!(MessageRef::decode(&original.encode()), Some(original));
    }
}

#[test]
fn the_encoding_is_three_parts_separated_by_one_space() {
    assert_eq!(sample(Channel::General).encode(), "general thr-1-2 msg-3-4");
}

#[test]
fn a_legacy_value_with_only_a_message_id_does_not_decode() {
    assert_eq!(MessageRef::decode("msg-3-4"), None);
    assert_eq!(MessageRef::decode("thr-1-2 msg-3-4"), None);
}

#[test]
fn hostile_or_malformed_values_do_not_decode() {
    let long = "a".repeat(200);
    for bad in [
        "",
        " ",
        "  ",
        "general  msg-1",
        "general thr-1 msg-1 extra",
        "general thr-1 msg-1 ",
        "general thr/1 msg-1",
        "general thr-1 msg 1",
        "../x thr-1 msg-1",
        "general thr-1 msg-1\n",
        "general thr-1\tmsg-1",
        "general thr-1 \u{e8}",
        &format!("general {long} msg-1"),
        &format!("{long} thr-1 msg-1"),
    ] {
        assert_eq!(MessageRef::decode(bad), None, "{bad:?}");
    }
}
