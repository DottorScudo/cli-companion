use super::*;
use crate::board::TaskStatus;
use crate::hub::events::diff::TaskChange;
use crate::hub::events::fixtures::{board, proposal, task};

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

fn at_rev(rev: u64, snapshot: BoardSnapshot) -> BoardSnapshot {
    BoardSnapshot { rev, ..snapshot }
}

#[test]
fn a_seeded_baseline_makes_the_first_change_after_enabling_an_event() {
    let mut baselines = Baselines::default();
    baselines.seed(at_rev(3, board(vec![task("a", TaskStatus::Queued)], vec![])));
    // La prima proposta dell'Intake crea il suo evento.
    let (_, events) =
        baselines.advance(at_rev(4, board(vec![task("a", TaskStatus::Queued)], vec![proposal("p", &["b"])])));
    assert!(matches!(events.as_slice(), [BoardEvent::ProposalCreated(p)] if p.id == "p"));
    // E cosi' il primo avvio di un task.
    let (_, events) =
        baselines.advance(at_rev(5, board(vec![task("a", TaskStatus::Running)], vec![proposal("p", &["b"])])));
    assert!(matches!(events.as_slice(), [BoardEvent::Task { change: TaskChange::Started, .. }]));
}

#[test]
fn seeding_keeps_a_base_already_published() {
    let mut baselines = Baselines::default();
    baselines.advance(at_rev(4, running()));
    // La vista letta all'accensione e' arrivata dopo una pubblicazione.
    baselines.seed(at_rev(4, board(vec![task("a", TaskStatus::Queued)], vec![])));
    assert!(baselines.advance(at_rev(5, running())).1.is_empty());
}

#[test]
fn a_view_older_than_the_base_is_ignored() {
    let mut baselines = Baselines::default();
    // Base letta dal registro prima che la revisione 4 fosse pubblicata.
    baselines.seed(at_rev(5, running()));
    let (base, events) = baselines.advance(at_rev(4, board(vec![task("a", TaskStatus::Queued)], vec![])));
    assert!(events.is_empty());
    assert_eq!(base.rev, 5);
    assert!(baselines.advance(at_rev(5, running())).1.is_empty());
}
