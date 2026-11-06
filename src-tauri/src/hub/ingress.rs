//! La porta d'ingresso dell'HUB per i messaggi che arrivano da fuori dell'app
//! (un'email, una chat): `deliver_external` scrive il messaggio di un mittente
//! esterno in un thread esistente o in uno nuovo, senza doppioni (stessa origine
//! nel thread indicato o, per un thread nuovo, in tutto il canale), e poi
//! notifica e, se richiesto, sveglia un segretario. Il testo di un mittente
//! esterno e' un dato non fidato: non passa mai da `send` ne' dalle menzioni;
//! solo `SecretaryEngine::on_external` lo porta al segretario, come dato.
//! Documentazione: docs/HUB.MD "API interna" e docs/CONNECTORS.MD.
//!
//! La parte pura (dedupe, thread nuovo o riaperto, testo, chi svegliare) e'
//! separata dall'I/O, cosi' si prova senza `AppHandle`:
//! - `check`: autore e origine coerenti (puro);
//! - `body`: il testo ripulito e tagliato (puro);
//! - `apply`: la scrittura nei thread del canale (puro).
//!
//! Bloccante: va chiamata fuori dal main thread. Gli allegati si preparano fuori
//! dal lock dell'HUB; se il messaggio non viene salvato (duplicato, errore) i file
//! tornano nello staging e si cancellano.

mod apply;
mod body;
mod check;

use super::attachments::{discard_staged_ids, skipped_note, stage_incoming, Claimed, IncomingFile};
use super::channel::Channel;
use super::commands::{emit_update, ensure_channel, ensure_enabled, hub_dirs_of};
use super::engine::SecretaryEngine;
use super::model::{Author, MessageOrigin};
use super::notify::{HubNotifier, NoticeKind};
use super::state::{HubState, MESSAGE_ID_PREFIX, THREAD_ID_PREFIX};
use apply::{apply, destination, precheck, Applied, Delivered, Stored, Write};
use tauri::{AppHandle, Manager};

/// Dove va il messaggio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryTarget {
    /// Un thread esistente del canale (se chiuso, si riapre).
    Existing(String),
    /// Un thread nuovo con questo titolo (ripulito; vuoto = "Senza titolo").
    New { title: String },
}

/// Chi svegliare dopo la consegna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalWake {
    None,
    /// Il segretario titolare del thread, se c'e'.
    Owner,
    /// Il segretario con questo nome.
    Secretary(String),
}

/// Un messaggio da consegnare. `author` deve essere `Author::External` e
/// `origin` dello stesso connettore e istanza; `files` sono gli allegati come byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDelivery {
    pub channel: Channel,
    pub target: DeliveryTarget,
    pub author: Author,
    pub body: String,
    pub files: Vec<IncomingFile>,
    pub origin: MessageOrigin,
    pub wake: ExternalWake,
}

/// L'esito: il thread e il messaggio (quello nuovo, o quello gia' consegnato se
/// `duplicate`: in quel caso non e' stato scritto, notificato o svegliato nulla).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryOutcome {
    pub thread_id: String,
    pub message_id: String,
    pub duplicate: bool,
}

impl DeliveryOutcome {
    fn duplicate(delivered: Delivered) -> Self {
        Self { thread_id: delivered.thread_id, message_id: delivered.message_id, duplicate: true }
    }
}

/// Consegna il messaggio. Errore con l'HUB spento, il canale o il thread
/// mancanti, un autore o un'origine non validi, un testo vuoto senza allegati.
pub fn deliver_external(app: &AppHandle, delivery: ExternalDelivery) -> Result<DeliveryOutcome, String> {
    ensure_enabled(app)?;
    check::check(&delivery)?;
    ensure_channel(app, &delivery.channel)?;
    let dirs = hub_dirs_of(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    let ExternalDelivery { channel, target, author, body, files, origin, wake } = delivery;
    let destination = destination(&target, &state.next_id(THREAD_ID_PREFIX, now));
    let seen = state.read(&dirs, &channel, |list| precheck(list, &destination, &origin))?;
    if let Some(delivered) = seen {
        return Ok(DeliveryOutcome::duplicate(delivered));
    }
    if files.is_empty() {
        body::compose(&body, None, false)?;
    }
    let staged = stage_incoming(app, files)?;
    let note = skipped_note(&staged.skipped);
    let body = body::compose(&body, note.as_deref(), !staged.ids.is_empty())
        .inspect_err(|_| discard_staged_ids(app, &staged.ids))?;
    let claimed = Claimed::take(&dirs.attachments, &channel, &destination.thread_id, &staged.ids)
        .inspect_err(|_| discard_staged_ids(app, &staged.ids))?;
    let message_id = state.next_id(MESSAGE_ID_PREFIX, now);
    let applied = state.update_when(
        &dirs,
        &channel,
        |list| {
            // Sotto il lock dell'HUB: una rimozione del workspace in corso aspetta.
            ensure_channel(app, &channel)?;
            let write = Write {
                channel: &channel,
                destination: &destination,
                author,
                origin,
                body,
                attachments: claimed.attachments().to_vec(),
                message_id: message_id.clone(),
                wake: &wake,
                now,
            };
            apply(list, write)
        },
        Applied::changed,
    );
    match applied {
        Ok(Applied::Stored(stored)) => {
            after_store(app, &channel, &message_id, &wake, stored);
            Ok(DeliveryOutcome { thread_id: destination.thread_id, message_id, duplicate: false })
        }
        Ok(Applied::Duplicate(delivered)) => {
            abandon(app, claimed, &staged.ids);
            Ok(DeliveryOutcome::duplicate(delivered))
        }
        Err(e) => {
            abandon(app, claimed, &staged.ids);
            Err(e)
        }
    }
}

/// Il messaggio non e' stato salvato: gli allegati tornano nello staging e li si cancella.
fn abandon(app: &AppHandle, claimed: Claimed, ids: &[String]) {
    claimed.release();
    discard_staged_ids(app, ids);
}

/// Dopo il salvataggio, lock rilasciato: l'evento per il frontend, la notifica e
/// il risveglio. Mai `on_message`: il testo esterno non e' una richiesta dell'utente.
fn after_store(app: &AppHandle, channel: &Channel, message_id: &str, wake: &ExternalWake, stored: Stored) {
    let thread_id = stored.summary.id.as_str();
    emit_update(app, &stored.summary);
    let kind = NoticeKind::External { name: stored.sender.clone() };
    app.state::<HubNotifier>().notify(app, &stored.summary, message_id, kind, &stored.body);
    match (&stored.secretary, wake) {
        (Some(secretary), _) => {
            app.state::<SecretaryEngine>().on_external(app, channel, thread_id, message_id, secretary);
        }
        (None, ExternalWake::Owner) => {
            log::info!("hub: il thread {thread_id} non ha un titolare, nessun segretario svegliato");
        }
        (None, ExternalWake::None | ExternalWake::Secretary(_)) => {}
    }
}
