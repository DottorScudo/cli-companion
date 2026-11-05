use super::*;

fn command(text: &str) -> Option<ContactCommand> {
    parse(text).command
}

#[test]
fn plain_text_is_not_a_command() {
    for text in ["", "ciao", " /help", "a /help", "😀"] {
        assert_eq!(parse(text), Parsed::default(), "{text:?}");
    }
}

#[test]
fn start_with_a_payload_is_a_pairing_invite() {
    let parsed = parse("/start ABC-def_123");
    assert_eq!(parsed, Parsed { pairing_code: Some("ABC-def_123".to_string()), command: None });
    assert_eq!(parse("/start@MuseBot XyZ").pairing_code.as_deref(), Some("XyZ"));
    assert_eq!(parse("/START code").pairing_code.as_deref(), Some("code"));
    assert_eq!(parse("/start   code  ").pairing_code.as_deref(), Some("code"));
    assert_eq!(parse("/start\ncode").pairing_code.as_deref(), Some("code"));
}

#[test]
fn start_without_a_valid_payload_is_help() {
    let too_long = format!("/start {}", "a".repeat(65));
    for text in ["/start", "/start ", "/start@bot", "/start a b", "/start a!", "/start é", too_long.as_str()] {
        assert_eq!(parse(text), Parsed { pairing_code: None, command: Some(ContactCommand::Help) }, "{text:?}");
    }
    assert!(valid_payload(&"a".repeat(64)));
}

#[test]
fn known_commands_map_to_their_variant() {
    assert_eq!(command("/help"), Some(ContactCommand::Help));
    assert_eq!(command("/threads"), Some(ContactCommand::Threads));
    assert_eq!(command("/use 2"), Some(ContactCommand::Use("2".to_string())));
    assert_eq!(command("/ws Progetto X"), Some(ContactCommand::Workspaces("Progetto X".to_string())));
    assert_eq!(command("/new  titolo lungo "), Some(ContactCommand::New("titolo lungo".to_string())));
    assert_eq!(command("/status"), Some(ContactCommand::Status));
    assert_eq!(command("/mute 2h"), Some(ContactCommand::Mute("2h".to_string())));
    assert_eq!(command("/mute"), Some(ContactCommand::Mute(String::new())));
    assert_eq!(command("/unmute"), Some(ContactCommand::Unmute));
    assert_eq!(command("/unlink"), Some(ContactCommand::Unlink));
    assert_eq!(command("/stop"), Some(ContactCommand::Unlink));
}

#[test]
fn the_name_is_case_insensitive_and_may_name_the_bot() {
    assert_eq!(command("/Threads@AnyBot"), Some(ContactCommand::Threads));
    assert_eq!(command("/USE@bot 3"), Some(ContactCommand::Use("3".to_string())));
}

#[test]
fn anything_else_is_unknown() {
    for text in ["/", "//", "/foo", "/foo bar", "/@bot", "/ä", "/threads2", "/😀"] {
        assert_eq!(command(text), Some(ContactCommand::Unknown), "{text:?}");
    }
}
