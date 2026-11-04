use super::*;
use crate::board::TaskStatus;
use crate::hub::events::diff::TaskChange;
use crate::hub::events::fixtures::{board, task};

fn running() -> BoardSnapshot {
    board(vec![task("a", TaskStatus::Running)], vec![])
}

#[test]
fn the_first_view_of_a_board_is_only_a_baseline() {
    let mut baselines = Baselines::default();
    // Dopo un riavvio i `running` sono gia' `failed`: nessun messaggio.
    let (_, events) = baselines.advance(board(vec![task("a", TaskStatus::Failed)], vec![]));
    assert!(events.is_empty());
    let (board_now, events) = baselines.advance(running());
    assert_eq!(board_now, &running());
    assert!(matches!(events.as_slice(), [BoardEvent::Task { change: TaskChange::Started, .. }]));
}

#[test]
fn clearing_or_forgetting_makes_the_next_view_a_baseline_again() {
    let mut baselines = Baselines::default();
    baselines.advance(board(vec![], vec![]));
    baselines.clear();
    assert!(baselines.advance(running()).1.is_empty());
    baselines.forget(crate::hub::events::fixtures::WS);
    assert!(baselines.advance(board(vec![], vec![])).1.is_empty());
}
