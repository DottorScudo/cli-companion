use super::*;
use crate::board::{schedule, SchedCtx, Task, TaskSpec};
use crate::workspaces::{Delivery, DeliveryMode, MergeMethod};

const FIRST: &str = "first";
const SECOND: &str = "second";

/// Board con i task `(id, agente)` in coda, nell'ordine dato.
fn board(tasks: &[(&str, &str)]) -> Board {
    let mut b = Board::new("ws-1");
    for (id, agent) in tasks {
        let spec = TaskSpec {
            id: id.to_string(),
            title: id.to_string(),
            task: format!("do {id}"),
            agent: Some(agent.to_string()),
            ..TaskSpec::default()
        };
        b.tasks.push(Task::from_spec(spec, 1));
    }
    b
}

fn gate(blocked: &[(&str, Option<Readiness>)]) -> AgentGate {
    let agents: Vec<&str> = blocked.iter().map(|(agent, _)| *agent).collect();
    AgentGate::build(agents, &mut |agent| blocked.iter().find(|(a, _)| *a == agent).and_then(|(_, r)| *r))
}

fn ctx(parallel: bool) -> SchedCtx {
    SchedCtx {
        delivery: Delivery { mode: DeliveryMode::Pr, parallel, merge_method: MergeMethod::Squash },
        gate_open: true,
    }
}

#[test]
fn build_keeps_only_agents_that_are_not_ready() {
    let mut asked = Vec::new();
    let gate = AgentGate::build([FIRST, SECOND, FIRST, "third"], &mut |agent| {
        asked.push(agent.to_string());
        match agent {
            FIRST => Some(Readiness::Ready),
            SECOND => Some(Readiness::NotFound),
            _ => None,
        }
    });
    assert_eq!(asked, ["first", "second", "third"], "ogni agente una volta sola");
    assert!(!gate.is_blocked(FIRST));
    assert_eq!((gate.is_blocked(SECOND), gate.reason(SECOND)), (true, Some(Readiness::NotFound)));
    assert_eq!((gate.is_blocked("third"), gate.reason("third")), (true, None), "in controllo: bloccato senza motivo");
    assert!(!gate.is_empty());
    assert!(AgentGate::build([FIRST], &mut |_| Some(Readiness::Ready)).is_empty());
}

#[test]
fn queued_agents_are_those_of_the_queued_tasks_only() {
    let mut b = board(&[("a", FIRST), ("b", SECOND), ("c", FIRST)]);
    b.start_task("b", 1).unwrap();
    assert_eq!(b.queued_agents().collect::<Vec<_>>(), [FIRST, FIRST]);
}

#[test]
fn a_blocked_agent_keeps_its_tasks_queued_and_lets_the_others_start() {
    let b = board(&[("a", FIRST), ("b", SECOND), ("c", SECOND)]);
    let nothing = AgentGate::default();
    assert_eq!(schedule(&b, &ctx(true), &nothing), ["a", "b", "c"]);
    let blocked = gate(&[(FIRST, Some(Readiness::NotInstalled))]);
    assert_eq!(schedule(&b, &ctx(true), &blocked), ["b", "c"]);
    // Anche mentre la prontezza e' in controllo.
    assert_eq!(schedule(&b, &ctx(true), &gate(&[(FIRST, None)])), ["b", "c"]);
}

#[test]
fn a_blocked_task_does_not_take_the_repository_slot() {
    // Stesso repository, un task alla volta: il bloccato non toglie il posto al secondo.
    let b = board(&[("a", FIRST), ("b", SECOND), ("c", SECOND)]);
    let blocked = gate(&[(FIRST, Some(Readiness::Outdated))]);
    assert_eq!(schedule(&b, &ctx(false), &AgentGate::default()), ["a"]);
    assert_eq!(schedule(&b, &ctx(false), &blocked), ["b"]);
}

#[test]
fn a_blocked_task_does_not_take_a_parallel_place() {
    let mut b = board(&[("a", FIRST), ("b", SECOND), ("c", SECOND)]);
    b.max_parallel = 1;
    let blocked = gate(&[(FIRST, Some(Readiness::Disabled))]);
    assert_eq!(schedule(&b, &ctx(true), &blocked), ["b"]);
}

#[test]
fn the_reason_is_shown_only_on_queued_tasks_with_a_known_reason() {
    let mut b = board(&[("a", FIRST), ("b", FIRST), ("c", SECOND)]);
    b.start_task("b", 1).unwrap();
    let blocked = gate(&[(FIRST, Some(Readiness::NotFound)), (SECOND, None)]);
    let task = |id: &str| b.task(id).unwrap();
    assert_eq!(
        blocked.unavailable(task("a")),
        Some(AgentUnavailableView { agent: FIRST.to_string(), reason: Readiness::NotFound })
    );
    assert_eq!(blocked.unavailable(task("b")), None, "in corso: niente motivo");
    assert_eq!(blocked.unavailable(task("c")), None, "ancora in controllo: niente motivo");
    assert_eq!(AgentGate::default().unavailable(task("a")), None);
}
