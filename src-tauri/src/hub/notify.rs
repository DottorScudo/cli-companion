//! Notifiche di sistema per i messaggi dei thread dell'Agents HUB. Il backend
//! decide cosa notificare e quando, ed emette l'evento Tauri `hub-notify` con
//! un `HubNotice`; il frontend decide se mostrarla (finestra in primo piano,
//! thread aperto e visibile) e mostra il toast. Documentazione: docs/HUB.MD,
//! "Notifiche".
//!
//! Notificano le risposte dei segretari (`append_text`), i messaggi dei mittenti
//! esterni (`ingress`) e alcuni eventi della board (`events::route`); mai i
//! messaggi dell'utente ne' gli avvisi del motore. Piu' candidati dello stesso
//! thread nella stessa finestra
//! (`GROUP_WINDOW`) diventano una sola notifica, la piu' significativa.
//! Oltre al frontend le notifiche raggruppate vanno ai `NoticeSink` registrati
//! (`set_notice_sink`), anche con l'interruttore delle notifiche spento.
//!
//! Un file per responsabilita':
//! - `notice`: i DTO verso il frontend (`HubNotice`, suono, riferimento al thread) e
//!   quello per i sink (`ContactNotice`, col tipo e il testo completo);
//! - `kind`: i tipi di notifica, con titolo, suono e significativita';
//! - `text`: il corpo (canale · thread, prima riga del messaggio), puro;
//! - `pending`: il raggruppamento per thread, puro;
//! - `sinks`: i destinatari oltre al frontend e la decisione di chi riceve, puro;
//! - `dispatch`: la colla con Tauri (`HubNotifier`, stato gestito): impostazioni,
//!   finestra di raggruppamento, consegna ai sink ed emissione.

mod dispatch;
mod kind;
mod notice;
mod pending;
mod sinks;
pub(super) mod text;

pub use dispatch::HubNotifier;
pub use kind::NoticeKind;
pub use notice::{validate_thread_ref, ContactNotice, HubThreadRef, NoticeCategory};
#[cfg(test)]
pub use notice::{HubNotice, HubNoticeSound};
pub use sinks::NoticeSink;
