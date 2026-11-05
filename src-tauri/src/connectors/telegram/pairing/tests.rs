use super::*;

#[test]
fn a_linked_account_gets_a_welcome_with_its_name() {
    let text = reply_text(&PairingOutcome::Linked { display_name: "Mario".to_string() });
    assert_eq!(text, "Collegato! Ciao Mario: da qui ricevi le notifiche dell'Agents HUB e scrivi ai segretari.");
}

#[test]
fn a_rejected_code_asks_for_a_new_one() {
    assert_eq!(reply_text(&PairingOutcome::Rejected), "Codice non valido o scaduto: generane uno nuovo dall'app.");
}

#[test]
fn too_many_attempts_asks_to_wait() {
    assert_eq!(
        reply_text(&PairingOutcome::TooManyAttempts),
        "Troppi tentativi di abbinamento: riprova tra qualche minuto."
    );
}
