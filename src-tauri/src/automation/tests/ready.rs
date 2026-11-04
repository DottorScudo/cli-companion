//! Segnale "frontend pronto" che sblocca il timer dell'automazione.

use super::auto::ReadySignal;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn wait_without_signal_times_out() {
    let sig = ReadySignal::new();
    let start = Instant::now();
    assert!(!sig.wait(Duration::from_millis(50)));
    assert!(start.elapsed() >= Duration::from_millis(50));
}

#[test]
fn signal_before_wait_returns_immediately() {
    let sig = ReadySignal::new();
    sig.signal();
    let start = Instant::now();
    assert!(sig.wait(Duration::from_secs(30)));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn signalling_twice_is_harmless() {
    let sig = ReadySignal::new();
    sig.signal();
    sig.signal();
    assert!(sig.wait(Duration::ZERO));
}

#[test]
fn signal_from_another_thread_wakes_the_waiter() {
    let sig = Arc::new(ReadySignal::new());
    let remote = Arc::clone(&sig);
    let start = Instant::now();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        remote.signal();
    });
    assert!(sig.wait(Duration::from_secs(30)));
    assert!(start.elapsed() < Duration::from_secs(5));
    t.join().unwrap();
}
