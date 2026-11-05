use super::*;

#[test]
fn the_help_lists_every_command_and_the_reply_rule() {
    let text = help();
    for command in [
        "/threads",
        "/use <n>",
        "/ws [n|nome]",
        "/new <titolo>",
        "/status",
        "/mute [minuti]",
        "/unmute",
        "/unlink",
        "/help",
    ] {
        assert!(text.contains(command), "{command}");
    }
    assert!(text.contains("rispondi a una notifica"));
}

#[test]
fn an_unknown_command_is_announced_before_the_help() {
    assert!(unknown_command().starts_with("Comando sconosciuto.\n\n"));
    assert!(unknown_command().ends_with(&help()));
}

#[test]
fn the_thread_list_is_numbered_with_a_marker_for_the_waiting_ones() {
    let lines = [ThreadLine { label: "Generale › A", waiting: true }, ThreadLine { label: "Sito › B", waiting: false }];
    assert_eq!(threads_list(&lines), "1. ⏳ Generale › A\n2. Sito › B\n\nScegli con /use <n>.");
}

#[test]
fn the_workspace_list_marks_the_chosen_one() {
    let lines = [ChannelLine { name: "Generale", chosen: false }, ChannelLine { name: "Sito", chosen: true }];
    let text = workspaces_list(&lines);
    assert!(text.starts_with("1. Generale\n2. ✅ Sito"));
}

#[test]
fn the_use_usage_depends_on_whether_a_list_was_shown() {
    assert!(use_usage(0).contains("/threads"));
    assert!(use_usage(4).contains("da 1 a 4"));
}

#[test]
fn the_status_reports_waiting_threads_and_the_mute_state() {
    let waiting = vec![("Generale".to_string(), 2), ("Sito".to_string(), 1)];
    let text = status("Generale › A", &waiting, MuteState::Minutes(5));
    assert_eq!(
        text,
        "Thread corrente: Generale › A\nThread che aspettano te: 3 (Generale: 2, Sito: 1).\nNotifiche silenziate ancora per 5 min."
    );
    let quiet = status("nessuno", &[], MuteState::Off);
    assert_eq!(quiet, "Thread corrente: nessuno\nNessun thread aspetta te.\nNotifiche attive.");
    assert!(status("x", &[], MuteState::Forever).ends_with("fino a /unmute."));
}

#[test]
fn skipped_attachments_name_the_reason_and_the_limits() {
    let skipped = [
        SkippedFile { name: "a.png".to_string(), reason: SkipReason::TooBig },
        SkippedFile { name: "b\n.txt".to_string(), reason: SkipReason::NotDownloaded },
        SkippedFile { name: "c".to_string(), reason: SkipReason::TooMany },
    ];
    assert_eq!(
        attachments_skipped(&skipped),
        "Allegati non importati: a.png (troppo grande), b .txt (download non riuscito), c (troppi allegati). Limiti: 10 MB per file, 5 per messaggio."
    );
}

#[test]
fn a_long_file_name_is_cut_on_a_character_boundary() {
    let skipped = [SkippedFile { name: "è".repeat(FILE_NAME_SHOWN_MAX + 5), reason: SkipReason::TooBig }];
    assert!(attachments_skipped(&skipped).contains(&format!("{}…", "è".repeat(FILE_NAME_SHOWN_MAX))));
}

#[test]
fn the_remaining_texts_are_in_italian_and_carry_their_arguments() {
    assert_eq!(thread_label("Sito", "Piano"), "Sito › Piano");
    assert_eq!(not_delivered("HUB spento"), "Messaggio non consegnato: HUB spento");
    assert_eq!(new_failed("titolo vuoto"), "Non ho creato il thread: titolo vuoto");
    assert!(mute_usage(10080).contains("1 a 10080"));
    assert!(muted_for(30).contains("30 min"));
    assert!(stale_reply().contains("non è più collegato"));
}
