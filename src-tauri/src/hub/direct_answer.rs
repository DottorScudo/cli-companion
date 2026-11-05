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
use super::attachments::commands::agent_paths;
use super::attachments::listing::{self, AttachmentPaths};
use super::channel::Channel;
use super::commands::hub_dirs_of;
use super::model::{Author, Message, Thread};
use super::questions::{replied_question, QuestionBoard};
use super::state::HubState;
use super::threads::THREAD_NOT_FOUND;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const ANSWER_SENT: &str = "Risposta inviata al worker.";
const NOT_PENDING: &str = "Risposta non inviata: questa domanda ha già una risposta o il suo task non è più in attesa.";
/// Il suggerimento dopo un errore di consegna.
const TERMINAL_HINT: &str = "Se serve, apri il terminale del worker dal Tasks Manager e rispondi lì.";
/// Prima dei percorsi degli allegati: lo legge il worker, quindi in inglese.
const ATTACHMENTS_HEADER: &str = "Attached files (read them at these absolute paths):";

/// L'esito della risposta diretta, se `message` risponde a una domanda del
/// thread: `None` se non e' una risposta a una domanda (il messaggio segue la
/// strada normale, segretari compresi). Gli allegati di `message` arrivano al
/// worker come percorsi assoluti nella forma di `attachments`, dopo il testo.
pub fn answer_reply(
    thread: &Thread,
    message: &Message,
    board: &impl QuestionBoard,
    attachments: AttachmentPaths<'_>,
) -> Option<String> {
    if message.author != Author::User {
        return None;
    }
    let target = match replied_question(thread, message.reply_to.as_deref()?)? {
        Ok(target) => target,
        Err(_) => return Some(NOT_PENDING.to_string()),
    };
    let answer = with_attachment_paths(thread, message, attachments);
    Some(match board.answer(&target, &answer) {
        Ok(()) => ANSWER_SENT.to_string(),
        Err(e) => undelivered(&e),
    })
}

/// Il testo della risposta cosi' com'e', e se ha allegati l'elenco dei loro
/// percorsi assoluti: il worker li legge dal disco. Senza allegati e' il testo.
fn with_attachment_paths(thread: &Thread, message: &Message, attachments: AttachmentPaths<'_>) -> String {
    if message.attachments.is_empty() {
        return message.body.clone();
    }
    let lines: Vec<String> = message
        .attachments
        .iter()
        .map(|a| format!("- {}", listing::describe_with_path(attachments, &thread.channel, &thread.id, a)))
        .collect();
    let list = format!("{ATTACHMENTS_HEADER}\n{}", lines.join("\n"));
    if message.body.trim().is_empty() {
        list
    } else {
        format!("{}\n\n{list}", message.body)
    }
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
    let (thread, attachments_root) = match load_thread(app, channel, thread_id) {
        Ok(loaded) => loaded,
        Err(e) => {
            log::warn!("hub: thread {thread_id} non letto per la risposta diretta: {e}");
            return false;
        }
    };
    // Le domande di un thread sono dei task del suo workspace: il worker che
    // riceve la risposta gira nell'ambiente del canale.
    let attachments = agent_paths(app, &attachments_root, channel);
    let Some(outcome) = answer_reply(&thread, message, &AppAnswers { app }, attachments) else {
        return false;
    };
    let notice = TextMessage { author: Author::System, body: outcome, reply_to: Some(message.id.clone()) };
    if let Err(e) = append_text(app, channel, thread_id, notice) {
        log::error!("hub: esito della risposta diretta non scritto nel thread {thread_id}: {e}");
    }
    true
}

/// Il thread e la radice degli allegati.
fn load_thread(app: &AppHandle, channel: &Channel, thread_id: &str) -> Result<(Thread, PathBuf), String> {
    let dirs = hub_dirs_of(app)?;
    let thread = app
        .state::<HubState>()
        .read(&dirs, channel, |list| list.thread(thread_id).cloned())
        .ok_or_else(|| THREAD_NOT_FOUND.to_string())?;
    Ok((thread, dirs.attachments))
}

#[cfg(test)]
mod tests;
