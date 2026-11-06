//! I testi (in italiano: li legge l'utente) della scheda di una richiesta di
//! azione e delle righe del suo esito.

use crate::hub::model::{ActionRequestState, HubActionRequest, RequestedAction, TaskField};

/// Una richiesta gia' decisa non si decide due volte.
pub const ALREADY_DECIDED: &str = "Questa richiesta è già stata decisa.";
/// La richiesta e' stata annullata dall'utente.
const CANCELLED: &str = "Richiesta annullata.";
/// La richiesta e' scaduta senza una decisione.
const EXPIRED: &str = "Richiesta scaduta: nessuna azione eseguita.";
/// Il task non c'e' piu' sulla board.
pub const TASK_GONE: &str = "Il task non c'è più sulla board.";
/// Il task ha cambiato stato dopo la richiesta.
pub const TASK_CHANGED: &str = "Il task ha cambiato stato.";
/// Il titolo, la specifica, le dipendenze o il modello sono cambiati dopo la richiesta.
pub const TASK_EDITED: &str = "Il task è stato modificato dopo la richiesta.";
/// L'azione non e' piu' ammessa dal task, per esempio per un dipendente in corso.
pub const ACTION_NOT_ALLOWED: &str = "L'azione non è più consentita su questo task.";
/// La nota di una richiesta scaduta.
pub const EXPIRED_NOTE: &str = "La richiesta è scaduta senza una decisione.";

/// Cosa chiede il segretario, all'infinito («eliminare»).
fn wish(action: RequestedAction) -> &'static str {
    match action {
        RequestedAction::Stop => "fermare",
        RequestedAction::Delete => "eliminare",
        RequestedAction::DeleteCascade => "eliminare",
        RequestedAction::MarkDone => "segnare come completato",
        RequestedAction::Update => "modificare",
    }
}

/// Cosa e' successo al task, al participio («eliminato»).
fn done(action: RequestedAction) -> &'static str {
    match action {
        RequestedAction::Stop => "fermato",
        RequestedAction::Delete => "eliminato",
        RequestedAction::DeleteCascade => "eliminato insieme ai task che ne dipendono",
        RequestedAction::MarkDone => "segnato come completato",
        RequestedAction::Update => "modificato",
    }
}

/// I campi che una modifica cambia: « (titolo e specifica)».
fn changed_fields(request: &HubActionRequest) -> String {
    let names: Vec<&str> = request.update.iter().flat_map(|u| &u.changes).map(|c| field_name(c.field)).collect();
    match names.as_slice() {
        [] => String::new(),
        [only] => format!(" ({only})"),
        [rest @ .., last] => format!(" ({} e {last})", rest.join(", ")),
    }
}

fn field_name(field: TaskField) -> &'static str {
    match field {
        TaskField::Title => "titolo",
        TaskField::Task => "specifica",
        TaskField::DependsOn => "dipendenze",
        TaskField::Model => "modello",
        TaskField::Bot => "Bot",
    }
}

/// Il corpo della scheda: «@ugo chiede di eliminare «Login»: <motivo>».
pub fn request_card(request: &HubActionRequest) -> String {
    let dependents = match (request.action, request.dependents.len()) {
        (RequestedAction::DeleteCascade, 1) => " e il task che ne dipende".to_string(),
        (RequestedAction::DeleteCascade, n) => format!(" e i {n} task che ne dipendono"),
        (RequestedAction::Update, _) => changed_fields(request),
        _ => String::new(),
    };
    format!(
        "@{} chiede di {} «{}»{dependents}: {}",
        request.secretary,
        wish(request.action),
        request.task_title,
        request.reason
    )
}

/// La riga dell'esito di una richiesta decisa.
pub fn outcome(request: &HubActionRequest) -> String {
    let note = request.note.as_deref().unwrap_or_default();
    match request.state {
        ActionRequestState::Confirmed => {
            format!("Richiesta confermata: «{}» {}.", request.task_title, done(request.action))
        }
        ActionRequestState::Cancelled => CANCELLED.to_string(),
        ActionRequestState::Expired => EXPIRED.to_string(),
        ActionRequestState::Stale => format!("Richiesta non più valida. {note} Nessuna azione eseguita."),
        ActionRequestState::Failed => format!("Richiesta confermata, ma la board non ha eseguito l'azione: {note}"),
        ActionRequestState::Pending => String::new(),
    }
}
