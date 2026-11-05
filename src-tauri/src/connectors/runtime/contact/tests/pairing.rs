use super::*;
use crate::connectors::contracts::PairingOutcome;
use crate::connectors::model::DISPLAY_NAME_MAX;
use crate::connectors::runtime::pairing::ATTEMPTS_MAX;

fn pairing_message(id: &str, account: &str, code: &str) -> InboundMessage {
    InboundMessage { pairing_code: Some(code.to_string()), ..inbound(id, account, "/start") }
}

fn outcomes(rig: &Rig) -> Vec<PairingOutcome> {
    rig.contact.pairing.lock().unwrap().iter().map(|(o, _)| o.clone()).collect()
}

#[test]
fn a_valid_code_links_the_account_and_the_help_follows() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "u1", &code)).unwrap();
    let links = rig.links();
    assert_eq!(links.len(), 1);
    assert_eq!((links[0].external_account.as_str(), links[0].external_chat.as_str()), ("u1", "chat-u1"));
    assert!(links[0].current.is_none() && links[0].channel.is_none() && links[0].id.starts_with("lnk-"));
    assert_eq!(
        *rig.contact.pairing.lock().unwrap(),
        vec![(PairingOutcome::Linked { display_name: "u1".to_string() }, "chat-u1".to_string())]
    );
    assert_eq!(rig.texts(), vec![replies::help()]);
    assert_eq!(rig.contact.sent.lock().unwrap()[0].chat, "chat-u1");
    // Lo stesso account che si riabbina non duplica il collegamento.
    let again = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m2", "u1", &again)).unwrap();
    assert_eq!(rig.links().len(), 1);
}

#[test]
fn the_display_name_comes_from_the_sender_name_cleaned() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    let message =
        InboundMessage { sender_name: "  Ma\u{7}rio\nRossi ".to_string(), ..pairing_message("m1", "u1", &code) };
    rig.inbound(message).unwrap();
    assert_eq!(rig.links()[0].display_name, "MarioRossi");
}

#[test]
fn without_a_sender_name_the_account_is_shown() {
    assert_eq!(linking::display_name("", "u1"), "u1");
    assert_eq!(linking::display_name(" \n ", "u\n1"), "u1");
    let long = "x".repeat(DISPLAY_NAME_MAX + 20);
    assert_eq!(linking::display_name(&long, "u1").chars().count(), DISPLAY_NAME_MAX);
    assert_eq!(linking::display_name("", &long).chars().count(), DISPLAY_NAME_MAX);
}

#[test]
fn a_wrong_or_reused_code_is_rejected_and_creates_nothing() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "u1", "SBAGLIATO123")).unwrap();
    rig.inbound(pairing_message("m2", "u1", &code)).unwrap();
    rig.inbound(pairing_message("m3", "u2", &code)).unwrap();
    assert_eq!(rig.links().len(), 1, "solo il secondo tentativo, col codice giusto");
    let outcomes = outcomes(&rig);
    assert_eq!(outcomes[0], PairingOutcome::Rejected);
    assert!(matches!(outcomes[1], PairingOutcome::Linked { .. }));
    assert_eq!(outcomes[2], PairingOutcome::Rejected, "il codice e' monouso");
    assert_eq!(rig.texts().len(), 1, "l'aiuto solo dopo l'abbinamento riuscito");
}

#[test]
fn an_expired_code_is_rejected() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.t.clock.advance(11 * 60 * 1000);
    rig.inbound(pairing_message("m1", "u1", &code)).unwrap();
    assert!(rig.links().is_empty());
    assert_eq!(outcomes(&rig), vec![PairingOutcome::Rejected]);
    assert!(rig.texts().is_empty());
}

#[test]
fn too_many_attempts_stop_even_the_right_code() {
    let rig = Rig::new(Vec::new());
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    for i in 0..ATTEMPTS_MAX {
        rig.inbound(pairing_message(&format!("w{i}"), "u9", "SBAGLIATO123")).unwrap();
    }
    rig.inbound(pairing_message("giusto", "u1", &code)).unwrap();
    assert!(rig.links().is_empty());
    assert_eq!(outcomes(&rig).last(), Some(&PairingOutcome::TooManyAttempts));
}

#[test]
fn the_link_limit_per_instance_is_respected() {
    let existing: Vec<ConnectorLink> = (1..=5).map(|i| link(&format!("lnk-1-{i}"), &format!("u{i}"), None)).collect();
    let rig = Rig::new(existing);
    let code = rig.pairing.issue(INSTANCE, &*rig.t.clock).unwrap();
    rig.inbound(pairing_message("m1", "nuovo", &code)).unwrap();
    assert_eq!(rig.links().len(), 5);
    assert_eq!(outcomes(&rig), vec![PairingOutcome::Rejected]);
}
