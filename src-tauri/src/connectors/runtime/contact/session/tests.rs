use super::*;

fn listed(n: usize) -> Vec<ListedThread> {
    (1..=n)
        .map(|i| ListedThread {
            channel: Channel::General,
            thread_id: format!("thr-{i}"),
            label: format!("Generale › {i}"),
        })
        .collect()
}

#[test]
fn the_thread_listing_is_addressed_from_one() {
    let mut s = AccountSession::default();
    assert_eq!(s.thread_at(1), None, "nessun elenco ricordato");
    s.remember_threads(listed(3));
    assert_eq!(s.listed_threads(), 3);
    assert_eq!(s.thread_at(0), None);
    assert_eq!(s.thread_at(1).map(|t| t.thread_id.as_str()), Some("thr-1"));
    assert_eq!(s.thread_at(3).map(|t| t.thread_id.as_str()), Some("thr-3"));
    assert_eq!(s.thread_at(4), None);
    s.remember_threads(listed(1));
    assert_eq!(s.thread_at(2), None, "un nuovo elenco sostituisce il vecchio");
}

#[test]
fn the_channel_listing_is_remembered() {
    let mut s = AccountSession::default();
    assert!(s.listed_channels().is_empty());
    s.remember_channels(vec![ChannelEntry { channel: Channel::General, name: "Generale".to_string() }]);
    assert_eq!(s.listed_channels().len(), 1);
}

#[test]
fn messages_within_the_limit_are_allowed() {
    let mut s = AccountSession::default();
    for i in 0..INBOUND_PER_MINUTE {
        assert_eq!(s.admit(i as i64), Admission::Allowed);
    }
}

#[test]
fn only_the_first_drop_of_a_window_is_announced() {
    let mut s = AccountSession::default();
    for _ in 0..INBOUND_PER_MINUTE {
        s.admit(0);
    }
    assert_eq!(s.admit(1), Admission::Dropped { first: true });
    assert_eq!(s.admit(2), Admission::Dropped { first: false });
    assert_eq!(s.admit(3), Admission::Dropped { first: false });
}

#[test]
fn the_window_rolls_and_a_later_overflow_is_announced_again() {
    let mut s = AccountSession::default();
    for _ in 0..INBOUND_PER_MINUTE {
        s.admit(0);
    }
    assert_eq!(s.admit(WINDOW_MS - 1), Admission::Dropped { first: true });
    assert_eq!(s.admit(WINDOW_MS), Admission::Allowed, "i messaggi di un minuto fa non contano piu'");
    for _ in 1..INBOUND_PER_MINUTE {
        assert_eq!(s.admit(WINDOW_MS), Admission::Allowed);
    }
    assert_eq!(s.admit(WINDOW_MS + 1), Admission::Dropped { first: true });
}

#[test]
fn dropped_messages_do_not_extend_the_window() {
    let mut s = AccountSession::default();
    for _ in 0..INBOUND_PER_MINUTE {
        s.admit(0);
    }
    for t in 1..50 {
        s.admit(t * 1000);
    }
    assert_eq!(s.admit(WINDOW_MS), Admission::Allowed);
}

#[test]
fn accounts_are_independent_and_can_be_forgotten() {
    let sessions = ContactSession::default();
    sessions.with("u1", |s| s.remember_threads(listed(2)));
    assert_eq!(sessions.with("u2", |s| s.listed_threads()), 0);
    assert_eq!(sessions.with("u1", |s| s.listed_threads()), 2);
    sessions.forget("u1");
    assert_eq!(sessions.with("u1", |s| s.listed_threads()), 0);
}
