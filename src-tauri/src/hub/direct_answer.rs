//! La risposta diretta a una domanda di un worker: un messaggio dell'utente con
//! `reply_to` su un evento "domanda" del thread ("Rispondi") e' la risposta, e
//! arriva al worker cosi' com'e', senza passare dal modello: deterministica,
//! gratuita e possibile anche in un thread senza segretario titolare. Un
//! messaggio cosi' non sveglia nessun segretario. L'esito e' un messaggio di
//! sistema in risposta al messaggio dell'utente.
//!
//! Il thread si legge prima (copia, lock rilasciato) e la consegna avviene fuori
//! da ogni lock dell'HUB: la board scrive nel terminale senza tenere i suoi.

use super::actions::AppAnswers;
use super::append::{append_text, TextMessage};
use super::channel::Channel;
use super::commands::hub_dirs_of;
use super::model::{Author, Message, Thread};
use super::questions::{replied_question, QuestionBoard};
use super::state::HubState;
use super::threads::THREAD_NOT_FOUND;
use tauri::{AppHandle, Manager};

pub const ANSWER_SENT: &str = "Risposta inviata al worker.";
const NOT_PENDING: &str = "Risposta non inviata: questa domanda ha già una risposta o il suo task non è più in attesa.";
/// Il suggerimento dopo un errore di consegna.
const TERMINAL_HINT: &str = "Se serve, apri il terminale del worker dal Tasks Manager e rispondi lì.";

/// L'esito della risposta diretta, se `message` risponde a una domanda del
/// thread: `None` se non e' una risposta a una domanda (il messaggio segue la
/// strada normale, segretari compresi).
pub fn answer_reply(thread: &Thread, message: &Message, board: &impl QuestionBoard) -> Option<String> {
    if message.author != Author::User {
        return None;
    }
    let target = match replied_question(thread, message.reply_to.as_deref()?)? {
        Ok(target) => target,
        Err(_) => return Some(NOT_PENDING.to_string()),
    };
    Some(match board.answer(&target, &message.body) {
        Ok(()) => ANSWER_SENT.to_string(),
        Err(e) => undelivered(&e),
    })
}

fn undelivered(reason: &str) -> String {
    format!("Risposta non inviata: {reason} {TERMINAL_HINT}")
}

/// `hub_send`: se il messaggio appena salvato risponde a una domanda, la
/// consegna e scrive l'esito nel thread. `true` = era una risposta (il
/// chiamante non sveglia i segretari). Bloccante: fuori dal main thread.
pub fn deliver_if_answer(app: &AppHandle, channel: &Channel, thread_id: &str, message: &Message) -> bool {
    if message.reply_to.is_none() {
        return false;
    }
    let thread = match load_thread(app, channel, thread_id) {
        Ok(thread) => thread,
        Err(e) => {
            log::warn!("hub: thread {thread_id} non letto per la risposta diretta: {e}");
            return false;
        }
    };
    let Some(outcome) = answer_reply(&thread, message, &AppAnswers { app }) else {
        return false;
    };
    let notice = TextMessage { author: Author::System, body: outcome, reply_to: Some(message.id.clone()) };
    if let Err(e) = append_text(app, channel, thread_id, notice) {
        log::error!("hub: esito della risposta diretta non scritto nel thread {thread_id}: {e}");
    }
    true
}

fn load_thread(app: &AppHandle, channel: &Channel, thread_id: &str) -> Result<Thread, String> {
    let dirs = hub_dirs_of(app)?;
    app.state::<HubState>()
        .read(&dirs, channel, |list| list.thread(thread_id).cloned())
        .ok_or_else(|| THREAD_NOT_FOUND.to_string())
}

#[cfg(test)]
mod tests;
