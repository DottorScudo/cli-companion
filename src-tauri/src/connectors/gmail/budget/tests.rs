use super::*;

#[test]
fn the_first_message_is_always_processed() {
    let budget = Budget::new(1000);
    assert!(!budget.time_up(1000 + ROUND_TIME_MAX_MS * 10));
}

#[test]
fn time_runs_out_after_the_first_message() {
    let mut budget = Budget::new(1000);
    budget.message_done();
    assert!(!budget.time_up(1000 + ROUND_TIME_MAX_MS - 1));
    assert!(budget.time_up(1000 + ROUND_TIME_MAX_MS));
}

#[test]
fn the_first_download_is_always_admitted() {
    assert!(Budget::new(0).admits(ROUND_BYTES_MAX * 2));
}

#[test]
fn bytes_run_out() {
    let mut budget = Budget::new(0);
    budget.spend(ROUND_BYTES_MAX - 10);
    assert!(budget.admits(10));
    assert!(!budget.admits(11));
    budget.spend(u64::MAX);
    assert!(!budget.admits(1));
}

#[test]
fn a_message_at_the_hub_limits_fits_in_the_budget() {
    use crate::hub::attachments::{ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};
    assert!(ATTACHMENTS_MAX as u64 * ATTACHMENT_SIZE_MAX <= ROUND_BYTES_MAX);
}
