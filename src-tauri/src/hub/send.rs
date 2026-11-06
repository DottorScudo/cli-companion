//! L'invio di un messaggio dell'utente a un thread, comune al comando `hub_send`
//! dell'app e ai canali di contatto (un connettore che porta i messaggi
//! dell'utente da un servizio esterno, vedi docs/CONNECTORS.MD): stessa strada
//! per tutti. Dopo il salvataggio, una risposta a una domanda di un worker
//! ancora in attesa va al worker cosi' com'e' (`direct_answer`, senza modello);
//! altrimenti il motore dei segretari decide chi svegliare: i menzionati o, senza
//! menzioni, l'autore del messaggio a cui si risponde, il titolare della sessione
//! attiva, il titolare del thread o l'ultimo segretario che vi ha scritto (anche
//! se dorme); se nessuno puo' rispondere, il thread riceve un avviso di sistema.
//! Un messaggio con `origin` non notifica mai, come quelli scritti nell'app.
//! Un messaggio con un `origin` gia' nel thread e' un doppione: non si scrive
//! di nuovo e non sveglia nessuno.
//!
//! Bloccante: va chiamato fuori dal main thread.

use super::append::{append_user_message, Appended, TextMessage};
use super::attachments::{discard_staged_ids, skipped_note, stage_incoming, IncomingFile};
use super::channel::Channel;
use super::commands::ensure_enabled;
use super::direct_answer::deliver_if_answer;
use super::engine::SecretaryEngine;
use super::model::{Author, Message, MessageOrigin};
use super::text::body_with_note;
use tauri::{AppHandle, Manager};

/// Il messaggio dell'utente da inviare: `reply_to` deve essere dello stesso
/// thread, `attachment_ids` sono allegati gia' nello staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessage {
    pub channel: Channel,
    pub thread_id: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub attachment_ids: Vec<String>,
    /// Da dove viene, se scritto da un canale di contatto (`None` nell'app).
    pub origin: Option<MessageOrigin>,
}

/// Invia il messaggio come utente. Rifiutato con l'HUB spento. Un messaggio con
/// un'origine gia' presente nel thread (un canale di contatto che lo ripresenta)
/// non si scrive ne' sveglia nessuno: si restituisce quello gia' salvato.
pub fn send_user_message(app: &AppHandle, message: UserMessage) -> Result<Message, String> {
    send(app, message).map(Appended::into_message)
}

fn send(app: &AppHandle, message: UserMessage) -> Result<Appended, String> {
    ensure_enabled(app)?;
    let UserMessage { channel, thread_id, body, reply_to, attachment_ids, origin } = message;
    let text = TextMessage { author: Author::User, body, reply_to };
    let appended = append_user_message(app, &channel, &thread_id, text, &attachment_ids, origin)?;
    if let Appended::Written(sent) = &appended {
        if !deliver_if_answer(app, &channel, &thread_id, sent) {
            app.state::<SecretaryEngine>().on_message(app, &channel, &thread_id, sent);
        }
    }
    Ok(appended)
}

/// Come `send_user_message`, con gli allegati come byte (da un canale di
/// contatto): `message.attachment_ids` deve essere vuoto. I file nei limiti
/// dell'app finiscono nello staging, gli altri si saltano e il messaggio finisce
/// con una riga "Allegati non importati: ..." (il testo si taglia per farle
/// posto). Se l'invio fallisce o il messaggio e' un doppione, i file nello
/// staging si cancellano.
pub fn send_user_files(app: &AppHandle, message: UserMessage, files: Vec<IncomingFile>) -> Result<Message, String> {
    ensure_enabled(app)?;
    if !message.attachment_ids.is_empty() {
        return Err("con i file in arrivo gli id degli allegati devono essere vuoti".to_string());
    }
    let staged = stage_incoming(app, files)?;
    let note = skipped_note(&staged.skipped);
    let message = UserMessage {
        body: body_with_note(&message.body, note.as_deref()),
        attachment_ids: staged.ids.clone(),
        ..message
    };
    match send(app, message) {
        Ok(Appended::Written(sent)) => Ok(sent),
        other => {
            discard_staged_ids(app, &staged.ids);
            other.map(Appended::into_message)
        }
    }
}
