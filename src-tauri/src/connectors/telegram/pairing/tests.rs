use super::*;

fn linked() -> PairingOutcome {
    PairingOutcome::Linked { display_name: "Mario".to_string() }
}

#[test]
fn a_linked_account_gets_a_welcome_with_its_name() {
    let text = reply_text(&linked(), false);
    assert_eq!(text, "Collegato! Ciao Mario: da qui ricevi le notifiche dell'Agents HUB e scrivi ai segretari.");
}

#[test]
fn a_group_welcome_warns_that_every_member_sees_the_notices() {
    let text = reply_text(&linked(), true);
    assert!(text.starts_with("Collegato! Ciao Mario: in questo gruppo"), "{text}");
    assert!(text.contains("ogni thread nel suo topic"));
    assert!(text.contains("tutti i membri del gruppo vedono le notifiche"));
    assert!(text.contains("solo te e il bot"));
}

#[test]
fn a_rejected_code_asks_for_a_new_one() {
    for in_group in [false, true] {
        assert_eq!(
            reply_text(&PairingOutcome::Rejected, in_group),
            "Codice non valido o scaduto: generane uno nuovo dall'app."
        );
    }
}

#[test]
fn too_many_attempts_asks_to_wait() {
    assert_eq!(
        reply_text(&PairingOutcome::TooManyAttempts, false),
        "Troppi tentativi di abbinamento: riprova tra qualche minuto."
    );
}
