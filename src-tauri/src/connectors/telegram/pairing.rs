//! La risposta a un tentativo di abbinamento (testi puri, invio con `sendMessage`).

use super::api;
use crate::connectors::contracts::{ConnectorContext, ConnectorError, PairingOutcome};

/// Il testo per l'utente. Dopo il benvenuto il runtime manda l'aiuto dei comandi.
pub fn reply_text(outcome: &PairingOutcome) -> String {
    match outcome {
        PairingOutcome::Linked { display_name } => {
            format!("Collegato! Ciao {display_name}: da qui ricevi le notifiche dell'Agents HUB e scrivi ai segretari.")
        }
        PairingOutcome::Rejected => "Codice non valido o scaduto: generane uno nuovo dall'app.".to_string(),
        PairingOutcome::TooManyAttempts => "Troppi tentativi di abbinamento: riprova tra qualche minuto.".to_string(),
    }
}

pub async fn reply(ctx: &ConnectorContext, outcome: &PairingOutcome, chat: &str) -> Result<(), ConnectorError> {
    let chat_id = chat.parse().map_err(|_| ConnectorError::Fatal("chat non valida".to_string()))?;
    api::send_message(ctx, chat_id, &reply_text(outcome), None).await.map(drop)
}

#[cfg(test)]
mod tests;
