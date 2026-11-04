//! L'aggancio dell'Agents HUB: `Host::board_published` riceve ogni revisione
//! davvero pubblicata, nell'ordine, e mai una revisione vecchia.

use super::*;

fn revs(views: &[BoardView]) -> Vec<(String, u64)> {
    views.iter().map(|v| (v.workspace_id.clone(), v.rev)).collect()
}

#[test]
fn every_published_revision_reaches_the_hub_hook_in_order() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    finish(&st, &h, &session_of(&st, &h, "a"), "completed");
    let published = h.published.lock().unwrap().clone();
    assert!(published.len() >= 3, "{published:?}");
    assert_eq!(published, revs(&h.board_events.lock().unwrap()));
}

#[test]
fn a_stale_revision_is_not_passed_to_the_hub_hook() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    let mut board = st.registry().boards["ws-1"].clone();
    let mut view = h.board_events.lock().unwrap().last().unwrap().clone();
    let (published, emitted) = (h.published.lock().unwrap().len(), h.board_events.lock().unwrap().len());
    for rev in [board.rev, board.rev - 1] {
        (board.rev, view.rev) = (rev, rev);
        let late = Effect::Publish(Box::new(board.clone()), Box::new(view.clone()));
        st.run(&h, |r| r.effects.push(late));
    }
    assert_eq!(h.published.lock().unwrap().len(), published);
    assert_eq!(h.board_events.lock().unwrap().len(), emitted);
}

#[test]
fn views_return_every_workspace_board_without_publishing() {
    let (st, h) = (AutomationState::default(), FakeHost::with_delivery(DeliveryMode::Commit, false));
    queue(&st, &h, vec![spec("a", &[])]);
    let (published, emitted) = (h.published.lock().unwrap().len(), h.board_events.lock().unwrap().len());
    let views = st.views(&h);
    let last = h.board_events.lock().unwrap().last().unwrap().clone();
    assert_eq!(revs(&views), vec![("ws-1".to_string(), last.rev), ("ws-2".to_string(), 0)]);
    assert_eq!(views[0].tasks.len(), last.tasks.len());
    assert_eq!(h.published.lock().unwrap().len(), published);
    assert_eq!(h.board_events.lock().unwrap().len(), emitted);
}
