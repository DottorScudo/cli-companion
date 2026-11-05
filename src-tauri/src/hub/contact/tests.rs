use super::*;

fn ws(id: &str, name: &str) -> (String, String) {
    (id.to_string(), name.to_string())
}

fn summary(id: &str, state: ThreadState, waiting_user: bool, updated_at: i64) -> ThreadSummary {
    ThreadSummary {
        id: id.to_string(),
        channel: Channel::General,
        title: format!("Titolo {id}"),
        state,
        task_count: 0,
        message_count: 0,
        waiting_user,
        owner: None,
        created_at: 0,
        updated_at,
    }
}

#[test]
fn general_comes_first_then_workspaces_by_name_ignoring_case() {
    let entries = channel_entries([ws("ws-1", "zeta"), ws("ws-2", "Alfa"), ws("ws-3", "beta")]);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Generale", "Alfa", "beta", "zeta"]);
    assert_eq!(entries[0].channel, Channel::General);
    assert_eq!(entries[1].channel, Channel::Workspace("ws-2".into()));
}

#[test]
fn without_workspaces_only_general_is_listed() {
    let entries = channel_entries(Vec::new());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].channel, Channel::General);
}

#[test]
fn a_workspace_name_is_cleaned_to_one_line() {
    let entries = channel_entries([ws("ws-1", "a\nb")]);
    assert_eq!(entries[1].name, "a b");
}

#[test]
fn only_open_threads_are_listed_with_their_flags() {
    let general = ChannelEntry { channel: Channel::General, name: "Generale".into() };
    let open = open_entries(
        &general,
        vec![
            summary("thr-1", ThreadState::Open, true, 30),
            summary("thr-2", ThreadState::Closed, false, 20),
            summary("thr-3", ThreadState::Open, false, 10),
        ],
    );
    let ids: Vec<&str> = open.iter().map(|t| t.thread_id.as_str()).collect();
    assert_eq!(ids, ["thr-1", "thr-3"]);
    assert!(open[0].waiting_user && !open[1].waiting_user);
    assert_eq!((open[0].updated_at, open[0].channel_name.as_str()), (30, "Generale"));
    assert_eq!(open[1].title, "Titolo thr-3");
}
