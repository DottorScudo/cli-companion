//! Notifiche di sistema per i messaggi dei thread dell'Agents HUB. Il backend
//! decide cosa notificare e quando, ed emette l'evento Tauri `hub-notify` con
//! un `HubNotice`; il frontend decide se mostrarla (finestra in primo piano,
//! thread aperto e visibile) e mostra il toast. Documentazione: docs/HUB.MD,
//! "Notifiche".
//!
//! Notificano le risposte dei segretari (`append_text`) e alcuni eventi della
//! board (`events::route`); mai i messaggi dell'utente ne' gli avvisi del
//! motore. Piu' candidati dello stesso thread nella stessa finestra
//! (`GROUP_WINDOW`) diventano una sola notifica, la piu' significativa.
//!
//! Un file per responsabilita':
//! - `notice`: i DTO verso il frontend (`HubNotice`, suono, riferimento al thread);
//! - `kind`: i tipi di notifica, con titolo, suono e significativita';
//! - `text`: il corpo (canale · thread, prima riga del messaggio), puro;
//! - `pending`: il raggruppamento per thread, puro;
//! - `dispatch`: la colla con Tauri (`HubNotifier`, stato gestito): impostazioni,
//!   finestra di raggruppamento ed emissione.

mod dispatch;
mod kind;
mod notice;
mod pending;
mod text;

pub use dispatch::HubNotifier;
pub use kind::NoticeKind;
pub use notice::validate_thread_ref;
