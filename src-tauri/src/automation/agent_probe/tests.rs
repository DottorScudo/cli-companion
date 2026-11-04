use super::*;

fn key() -> Key {
    ("windows".to_string(), "agent".to_string())
}

#[test]
fn the_first_lookup_knows_nothing_and_starts_one_probe() {
    let (probes, t0) = (AgentProbes::default(), Instant::now());
    assert_eq!(probes.lookup(&key(), t0), Lookup { known: None, start_probe: true });
    // Il rinnovo e' in corso: nessun secondo thread.
    assert_eq!(probes.lookup(&key(), t0), Lookup { known: None, start_probe: false });
}

#[test]
fn a_recorded_value_is_returned_and_refreshed_only_when_stale() {
    let (probes, t0) = (AgentProbes::default(), Instant::now());
    probes.lookup(&key(), t0);
    assert!(probes.record(key(), Readiness::NotFound, t0), "da sconosciuto a noto e' un cambiamento");
    let fresh = t0 + REFRESH_AFTER - Duration::from_millis(1);
    assert_eq!(probes.lookup(&key(), fresh), Lookup { known: Some(Readiness::NotFound), start_probe: false });
    // Scaduto: risponde con il valore vecchio e ne chiede uno nuovo, una volta sola.
    let stale = t0 + REFRESH_AFTER;
    assert_eq!(probes.lookup(&key(), stale), Lookup { known: Some(Readiness::NotFound), start_probe: true });
    assert_eq!(probes.lookup(&key(), stale), Lookup { known: Some(Readiness::NotFound), start_probe: false });
}

#[test]
fn record_reports_a_change_only_when_the_value_differs() {
    let (probes, t0) = (AgentProbes::default(), Instant::now());
    assert!(probes.record(key(), Readiness::Ready, t0));
    assert!(!probes.record(key(), Readiness::Ready, t0));
    assert!(probes.record(key(), Readiness::Outdated, t0));
}

#[test]
fn a_probe_that_never_finishes_does_not_block_the_next_ones_forever() {
    let (probes, t0) = (AgentProbes::default(), Instant::now());
    assert!(probes.lookup(&key(), t0).start_probe);
    assert!(!probes.lookup(&key(), t0 + PROBE_STUCK_AFTER - Duration::from_millis(1)).start_probe);
    assert!(probes.lookup(&key(), t0 + PROBE_STUCK_AFTER).start_probe);
}

#[test]
fn values_are_kept_per_target_and_agent() {
    let (probes, t0) = (AgentProbes::default(), Instant::now());
    probes.record(key(), Readiness::Ready, t0);
    let other_target = ("Ubuntu".to_string(), "agent".to_string());
    assert_eq!(probes.lookup(&other_target, t0), Lookup { known: None, start_probe: true });
    let other_agent = ("windows".to_string(), "other".to_string());
    assert_eq!(probes.lookup(&other_agent, t0), Lookup { known: None, start_probe: true });
}
