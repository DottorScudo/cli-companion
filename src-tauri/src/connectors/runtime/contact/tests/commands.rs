use super::*;
use crate::connectors::model::LinkThread;
use crate::connectors::testing::command;
use crate::hub::contact::ChannelEntry;

fn ws(id: &str) -> Channel {
    Channel::Workspace(id.to_string())
}

fn cmd(id: &str, c: ContactCommand) -> InboundMessage {
    command(id, "u1", c)
}

/// `thr-1` (Generale, aspetta l'utente), `thr-2` (Generale) e `thr-9` (workspace Sito).
fn with_threads(rig: &Rig) {
    rig.t.hub.add_open_thread(open_thread(Channel::General, "Generale", "thr-1", true, 10));
    rig.t.hub.add_open_thread(open_thread(Channel::General, "Generale", "thr-2", false, 30));
    rig.t.hub.add_open_thread(open_thread(ws("ws-1"), "Sito", "thr-9", false, 20));
}

fn with_channels(rig: &Rig) {
    rig.t.hub.set_channels(vec![
        ChannelEntry { channel: Channel::General, name: "Generale".to_string() },
        ChannelEntry { channel: ws("ws-1"), name: "Sito".to_string() },
        ChannelEntry { channel: ws("ws-2"), name: "Altro".to_string() },
    ]);
}

#[test]
fn help_and_unknown_commands_reply_with_the_command_list() {
    let rig = Rig::linked();
    rig.run(vec![cmd("c1", ContactCommand::Help), cmd("c2", ContactCommand::Unknown)]).unwrap();
    assert_eq!(rig.texts(), vec![replies::help(), replies::unknown_command()]);
    assert!(rig.t.hub.calls().is_empty());
}

#[test]
fn threads_lists_waiting_first_then_recent_and_use_picks_one() {
    let rig = Rig::linked();
    with_threads(&rig);
    rig.run(vec![cmd("c1", ContactCommand::Threads), cmd("c2", ContactCommand::Use("3".to_string()))]).unwrap();
    let texts = rig.texts();
    assert_eq!(
        texts[0],
        "1. ⏳ Generale › Titolo thr-1\n2. Generale › Titolo thr-2\n3. Sito › Titolo thr-9\n\nScegli con /use <n>."
    );
    assert_eq!(texts[1], "Thread corrente: Sito › Titolo thr-9");
    let saved = rig.first_link();
    assert_eq!(saved.current, Some(LinkThread { channel: ws("ws-1"), thread_id: "thr-9".to_string() }));
    assert_eq!(saved.channel, Some(ws("ws-1")), "/use sceglie anche il canale dei thread nuovi");
}

#[test]
fn threads_without_open_threads_suggests_new() {
    let rig = Rig::linked();
    rig.inbound(cmd("c1", ContactCommand::Threads)).unwrap();
    assert!(rig.texts()[0].contains("/new"));
}

#[test]
fn use_needs_a_number_within_the_last_listing() {
    let rig = Rig::linked();
    with_threads(&rig);
    let use_ = |id: &str, arg: &str| cmd(id, ContactCommand::Use(arg.to_string()));
    rig.run(vec![
        use_("c0", "1"),
        cmd("c1", ContactCommand::Threads),
        use_("c2", "0"),
        use_("c3", "4"),
        use_("c4", "uno"),
        use_("c5", ""),
    ])
    .unwrap();
    let texts = rig.texts();
    assert_eq!(texts[0], replies::use_usage(0), "senza un elenco");
    assert_eq!(
        texts[2..],
        [replies::use_usage(3), replies::use_usage(3), replies::use_usage(3), replies::use_usage(3)]
    );
    assert!(rig.first_link().channel.is_none(), "nulla e' stato scelto");
}

#[test]
fn use_refuses_a_thread_that_no_longer_exists() {
    let rig = Rig::linked();
    with_threads(&rig);
    rig.t.hub.threads.lock().unwrap().remove(&("general".to_string(), "thr-2".to_string()));
    rig.run(vec![cmd("c1", ContactCommand::Threads), cmd("c2", ContactCommand::Use("2".to_string()))]).unwrap();
    assert_eq!(rig.texts()[1], replies::thread_gone());
    assert_eq!(rig.first_link().current.map(|c| c.thread_id), Some("thr-1".to_string()));
}

#[test]
fn a_hub_error_while_listing_is_reported() {
    let rig = Rig::linked();
    *rig.t.hub.list_error.lock().unwrap() = Some("L'Agents HUB è spento".to_string());
    rig.run(vec![
        cmd("c1", ContactCommand::Threads),
        cmd("c2", ContactCommand::Workspaces(String::new())),
        cmd("c3", ContactCommand::Status),
    ])
    .unwrap();
    assert_eq!(rig.texts(), vec!["L'Agents HUB è spento"; 3]);
}

#[test]
fn ws_lists_the_channels_marking_the_chosen_one_and_picks_by_number_or_name() {
    let rig = Rig::linked();
    with_channels(&rig);
    rig.run(vec![
        cmd("c1", ContactCommand::Workspaces(String::new())),
        cmd("c2", ContactCommand::Workspaces("2".to_string())),
        cmd("c3", ContactCommand::Workspaces(String::new())),
        cmd("c4", ContactCommand::Workspaces(" altro ".to_string())),
    ])
    .unwrap();
    let texts = rig.texts();
    assert!(texts[0].starts_with("1. ✅ Generale\n2. Sito\n3. Altro"), "{}", texts[0]);
    assert_eq!(texts[1], replies::workspace_chosen("Sito"));
    assert!(texts[2].contains("2. ✅ Sito"), "{}", texts[2]);
    assert_eq!(texts[3], replies::workspace_chosen("Altro"));
    assert_eq!(rig.first_link().channel, Some(ws("ws-2")));
}

#[test]
fn ws_with_an_unknown_name_or_number_changes_nothing() {
    let rig = Rig::linked();
    with_channels(&rig);
    rig.run(vec![
        cmd("c1", ContactCommand::Workspaces("nessuno".to_string())),
        cmd("c2", ContactCommand::Workspaces("2".to_string())),
        cmd("c3", ContactCommand::Workspaces(String::new())),
        cmd("c4", ContactCommand::Workspaces("9".to_string())),
    ])
    .unwrap();
    let texts = rig.texts();
    assert_eq!(texts[0], replies::workspace_not_found());
    assert_eq!(texts[1], replies::workspace_not_found(), "il numero vale solo dopo aver visto l'elenco");
    assert_eq!(texts[3], replies::workspace_not_found());
    assert!(rig.first_link().channel.is_none());
}

#[test]
fn new_opens_a_thread_in_the_right_channel_and_makes_it_current() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    rig.inbound(cmd("c1", ContactCommand::New("Idea".to_string()))).unwrap();
    assert_eq!(rig.t.hub.calls(), vec![HubCall::CreateThread { channel: Channel::General, title: "Idea".to_string() }]);
    let saved = rig.first_link();
    assert_eq!(saved.current, Some(LinkThread { channel: Channel::General, thread_id: "thr-new-1".to_string() }));
    assert_eq!(rig.texts(), vec![replies::thread_created("Idea")]);
}

#[test]
fn new_prefers_the_chosen_channel_then_the_one_of_the_current_thread() {
    let mut chosen = link("lnk-1-1", "u1", Some("thr-1"));
    chosen.channel = Some(ws("ws-1"));
    let rig = Rig::new(vec![chosen]);
    rig.t.hub.add_thread(&ws("ws-1"), "thr-x");
    rig.inbound(cmd("c1", ContactCommand::New("A".to_string()))).unwrap();
    let mut on_ws = link("lnk-1-1", "u1", None);
    on_ws.current = Some(LinkThread { channel: ws("ws-2"), thread_id: "thr-y".to_string() });
    let rig2 = Rig::new(vec![on_ws]);
    rig2.inbound(cmd("c1", ContactCommand::New("B".to_string()))).unwrap();
    assert!(matches!(&rig.t.hub.calls()[..], [HubCall::CreateThread { channel, .. }] if *channel == ws("ws-1")));
    assert!(matches!(&rig2.t.hub.calls()[..], [HubCall::CreateThread { channel, .. }] if *channel == ws("ws-2")));
    assert_eq!(rig.first_link().current.map(|c| c.channel), Some(ws("ws-1")));
}

#[test]
fn new_without_a_title_explains_the_usage_and_a_hub_error_is_reported() {
    let rig = Rig::linked();
    rig.inbound(cmd("c1", ContactCommand::New("  ".to_string()))).unwrap();
    assert_eq!(rig.texts(), vec![replies::new_usage()]);
    assert!(rig.t.hub.calls().is_empty());
    *rig.t.hub.fail_with.lock().unwrap() = Some("L'Agents HUB è spento".to_string());
    rig.inbound(cmd("c2", ContactCommand::New("Idea".to_string()))).unwrap();
    assert_eq!(rig.texts()[1], replies::new_failed("L'Agents HUB è spento"));
    assert_eq!(rig.first_link().current.map(|c| c.thread_id), Some("thr-1".to_string()), "invariato");
}

#[test]
fn status_reports_current_thread_waiting_threads_and_mute() {
    let mut muted = link("lnk-1-1", "u1", Some("thr-1"));
    muted.muted_until = Some(NOW + 90_000);
    let rig = Rig::new(vec![muted]);
    with_threads(&rig);
    rig.t.hub.add_open_thread(open_thread(ws("ws-1"), "Sito", "thr-10", true, 5));
    rig.inbound(cmd("c1", ContactCommand::Status)).unwrap();
    assert_eq!(
        rig.texts(),
        vec![
            "Thread corrente: Generale › Titolo thr-1\nThread che aspettano te: 2 (Generale: 1, Sito: 1).\nNotifiche silenziate ancora per 2 min."
        ]
    );
}

#[test]
fn status_without_a_current_thread_says_none() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", None)]);
    rig.inbound(cmd("c1", ContactCommand::Status)).unwrap();
    assert_eq!(rig.texts(), vec!["Thread corrente: nessuno\nNessun thread aspetta te.\nNotifiche attive."]);
}

#[test]
fn mute_unmute_and_their_usage() {
    let rig = Rig::linked();
    let mute = |id: &str, arg: &str| cmd(id, ContactCommand::Mute(arg.to_string()));
    rig.inbound(mute("c1", "")).unwrap();
    assert_eq!(rig.first_link().muted_until, Some(i64::MAX));
    rig.inbound(mute("c2", "30")).unwrap();
    assert_eq!(rig.first_link().muted_until, Some(NOW + 30 * 60_000));
    rig.inbound(mute("c3", "0")).unwrap();
    rig.inbound(mute("c4", "10081")).unwrap();
    rig.inbound(mute("c5", "tanto")).unwrap();
    assert_eq!(rig.first_link().muted_until, Some(NOW + 30 * 60_000), "gli usi sbagliati non cambiano nulla");
    rig.inbound(cmd("c6", ContactCommand::Unmute)).unwrap();
    assert_eq!(rig.first_link().muted_until, None);
    let texts = rig.texts();
    assert_eq!(texts[0], replies::muted_forever());
    assert_eq!(texts[1], replies::muted_for(30));
    assert_eq!(texts[2..5], vec![replies::mute_usage(7 * 24 * 60); 3]);
    assert_eq!(texts[5], replies::unmuted());
}

#[test]
fn mute_up_to_seven_days_is_accepted() {
    let rig = Rig::linked();
    rig.inbound(cmd("c1", ContactCommand::Mute((7 * 24 * 60).to_string()))).unwrap();
    assert_eq!(rig.first_link().muted_until, Some(NOW + 7 * 24 * 60 * 60_000));
}

#[test]
fn unlink_replies_first_and_then_removes_the_link() {
    let rig = Rig::new(vec![link("lnk-1-1", "u1", Some("thr-1")), link("lnk-1-2", "u2", None)]);
    rig.inbound(cmd("c1", ContactCommand::Unlink)).unwrap();
    assert_eq!(rig.texts(), vec![replies::unlinked()]);
    assert_eq!(rig.contact.sent.lock().unwrap()[0].chat, "chat-u1");
    let remaining: Vec<String> = rig.links().into_iter().map(|l| l.external_account).collect();
    assert_eq!(remaining, ["u2"]);
    // Da ora l'account e' uno sconosciuto: silenzio.
    rig.inbound(inbound("m2", "u1", "ancora qui?")).unwrap();
    assert_eq!(rig.texts().len(), 1);
    assert!(rig.t.hub.calls().is_empty());
}
