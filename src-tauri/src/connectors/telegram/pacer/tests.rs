use super::*;

#[test]
fn the_first_message_goes_out_at_once() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.reserve("1", 1_000), 0);
}

#[test]
fn messages_to_the_same_chat_are_spaced() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.reserve("1", 1_000), 0);
    assert_eq!(pacer.reserve("1", 1_000), SLOT_GAP_MS);
    assert_eq!(pacer.reserve("1", 1_000), 2 * SLOT_GAP_MS);
}

#[test]
fn waiting_already_elapsed_is_not_waited_twice() {
    let mut pacer = Pacer::new();
    pacer.reserve("1", 0);
    assert_eq!(pacer.reserve("1", 400), SLOT_GAP_MS - 400);
    assert_eq!(pacer.reserve("1", 10 * SLOT_GAP_MS), 0, "la chat e' di nuovo libera");
}

#[test]
fn chats_are_independent() {
    let mut pacer = Pacer::new();
    pacer.reserve("1", 0);
    assert_eq!(pacer.reserve("2", 0), 0);
}

#[test]
fn the_clock_going_backwards_never_gives_a_negative_wait() {
    let mut pacer = Pacer::new();
    pacer.reserve("1", 5_000);
    assert!(pacer.reserve("1", 1_000) >= 0);
}

#[test]
fn extreme_times_do_not_overflow() {
    let mut pacer = Pacer::new();
    pacer.reserve("1", i64::MAX);
    assert!(pacer.reserve("1", i64::MAX) >= 0);
    assert!(pacer.reserve("1", i64::MIN) >= 0);
}

#[test]
fn the_map_is_bounded_and_forgets_the_oldest_chat() {
    let mut pacer = Pacer::new();
    for chat in 0..(CHATS_MAX as i64 + 20) {
        pacer.reserve(&chat.to_string(), chat);
    }
    assert_eq!(pacer.next_free.len(), CHATS_MAX);
    assert!(!pacer.next_free.contains_key("0"));
    assert!(pacer.next_free.contains_key(&(CHATS_MAX as i64 + 19).to_string()));
}

const FORUM: &str = "-1001234567890";

#[test]
fn group_chats_are_spaced_about_three_seconds() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.reserve(FORUM, 1_000), 0);
    assert_eq!(pacer.reserve(FORUM, 1_000), GROUP_SLOT_GAP_MS);
    assert_eq!(pacer.reserve(FORUM, 1_000), 2 * GROUP_SLOT_GAP_MS);
    assert_eq!(pacer.reserve("-5", 0), 0);
    assert_eq!(pacer.reserve("-5", 0), GROUP_SLOT_GAP_MS, "anche un gruppo base");
}

#[test]
fn twenty_group_calls_take_about_a_minute() {
    let mut pacer = Pacer::new();
    let waits: Vec<i64> = (0..20).map(|_| pacer.reserve(FORUM, 0)).collect();
    assert!(*waits.last().unwrap() >= 57_000, "{waits:?}");
}

#[test]
fn private_chats_keep_their_one_second_pace_next_to_a_group() {
    let mut pacer = Pacer::new();
    pacer.reserve(FORUM, 0);
    pacer.reserve("42", 0);
    assert_eq!(pacer.reserve("42", 0), SLOT_GAP_MS);
    assert_eq!(pacer.reserve(FORUM, 0), GROUP_SLOT_GAP_MS);
}
