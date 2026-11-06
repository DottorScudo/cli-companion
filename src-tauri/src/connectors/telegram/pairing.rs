//! La risposta a un tentativo di abbinamento (testi puri, invio con `sendMessage`).

use super::forum::is_group;
use super::pacer::Pacer;
use super::send::send;
use crate::connectors::contracts::{ConnectorContext, ConnectorError, OutboundMessage, PairingOutcome};
use std::sync::Mutex;

/// Nel benvenuto di un gruppo: chi vede le notifiche.
const GROUP_VISIBILITY_WARNING: &str = "Attenzione: tutti i membri del gruppo vedono le notifiche e i messaggi. \
    Tieni nel gruppo solo te e il bot.";

/// Il testo per l'utente, dalla chat privata o da un gruppo forum. Dopo il benvenuto il
/// runtime manda l'aiuto dei comandi.
pub fn reply_text(outcome: &PairingOutcome, in_group: bool) -> String {
    match outcome {
        PairingOutcome::Linked { display_name } if in_group => format!(
            "Collegato! Ciao {display_name}: in questo gruppo ricevi le notifiche dell'Agents HUB, ogni thread nel \
             suo topic, e scrivi ai segretari. {GROUP_VISIBILITY_WARNING}"
        ),
        PairingOutcome::Linked { display_name } => {
            format!("Collegato! Ciao {display_name}: da qui ricevi le notifiche dell'Agents HUB e scrivi ai segretari.")
        }
        PairingOutcome::Rejected => "Codice non valido o scaduto: generane uno nuovo dall'app.".to_string(),
        PairingOutcome::TooManyAttempts => "Troppi tentativi di abbinamento: riprova tra qualche minuto.".to_string(),
    }
}

pub async fn reply(
    pacer: &Mutex<Pacer>,
    ctx: &ConnectorContext,
    outcome: &PairingOutcome,
    chat: &str,
) -> Result<(), ConnectorError> {
    let out = OutboundMessage {
        chat: chat.to_string(),
        subchat: None,
        text: reply_text(outcome, is_group(chat)),
        reply_to_external: None,
    };
    send(pacer, ctx, &out).await.map(drop)
}

#[cfg(test)]
mod tests;
