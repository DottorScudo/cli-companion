use super::*;
use WorkerActivity::{Background, Idle, NeedsInput, Running, Starting};

const ACTIVITIES: [WorkerActivity; 5] = [Starting, Running, Background, NeedsInput, Idle];

fn step(to: Option<WorkerActivity>, block_stop: bool) -> WorkerStep {
    WorkerStep { to, block_stop }
}

fn turn_ended(background_work: bool, stop_hook_active: bool) -> WorkerSignal {
    WorkerSignal::TurnEnded { background_work, stop_hook_active }
}

/// Lo stato che la tabella produce da `from` con `signal`.
fn on(from: WorkerActivity, signal: WorkerSignal) -> WorkerStep {
    from.on(signal)
}

#[test]
fn a_permission_prompt_moves_every_active_worker_to_needs_input() {
    for from in ACTIVITIES {
        let expected = if from == NeedsInput { None } else { Some(NeedsInput) };
        assert_eq!(on(from, WorkerSignal::PermissionPrompt), step(expected, false), "{from:?}");
    }
}

#[test]
fn a_turn_ended_with_background_work_moves_to_background_and_never_blocks() {
    for from in ACTIVITIES {
        for stop_hook_active in [false, true] {
            let expected = if from == Background { None } else { Some(Background) };
            assert_eq!(on(from, turn_ended(true, stop_hook_active)), step(expected, false), "{from:?}");
        }
    }
}

#[test]
fn the_first_turn_end_without_a_report_blocks_the_stop_and_resumes_a_waiting_worker() {
    for from in ACTIVITIES {
        let expected = from.needs_attention().then_some(Running);
        assert_eq!(on(from, turn_ended(false, false)), step(expected, true), "{from:?}");
    }
}

#[test]
fn the_second_turn_end_without_a_report_leaves_the_worker_idle() {
    for from in ACTIVITIES {
        let expected = if from == Idle { None } else { Some(Idle) };
        assert_eq!(on(from, turn_ended(false, true)), step(expected, false), "{from:?}");
    }
}

#[test]
fn user_input_resumes_only_a_worker_that_was_waiting() {
    for from in ACTIVITIES {
        let expected = from.needs_attention().then_some(Running);
        assert_eq!(on(from, WorkerSignal::UserInput), step(expected, false), "{from:?}");
    }
}

#[test]
fn reading_the_task_moves_only_a_starting_worker_to_running() {
    for from in ACTIVITIES {
        let expected = (from == Starting).then_some(Running);
        assert_eq!(on(from, WorkerSignal::TaskRead), step(expected, false), "{from:?}");
    }
}

#[test]
fn only_waiting_activities_need_attention() {
    let waiting: Vec<_> = ACTIVITIES.into_iter().filter(|a| a.needs_attention()).collect();
    assert_eq!(waiting, [NeedsInput, Idle]);
}

#[test]
fn the_wire_status_maps_to_and_from_the_activity_and_the_end() {
    for a in ACTIVITIES {
        let status = WorkerStatus::from(a);
        assert_eq!(status.activity(), Some(a));
    }
    for (end, status) in [
        (WorkerEnd::Completed, WorkerStatus::Completed),
        (WorkerEnd::Failed, WorkerStatus::Failed),
        (WorkerEnd::Closed, WorkerStatus::Closed),
    ] {
        assert_eq!(WorkerStatus::from(end), status);
        assert_eq!(status.activity(), None);
    }
}

#[test]
fn worker_statuses_keep_their_wire_strings() {
    let names = ["starting", "running", "background", "needs_input", "idle", "completed", "failed", "closed"];
    let all = [
        WorkerStatus::Starting,
        WorkerStatus::Running,
        WorkerStatus::Background,
        WorkerStatus::NeedsInput,
        WorkerStatus::Idle,
        WorkerStatus::Completed,
        WorkerStatus::Failed,
        WorkerStatus::Closed,
    ];
    for (status, name) in all.into_iter().zip(names) {
        assert_eq!(serde_json::to_value(status).unwrap(), name);
        assert_eq!(serde_json::from_value::<WorkerStatus>(name.into()).unwrap(), status);
    }
}
