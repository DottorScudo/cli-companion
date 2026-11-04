//! Gli eventi della board nei thread dell'HUB: con l'HUB acceso, cio' che
//! succede sulla board di un workspace diventa messaggi evento nei thread del
//! suo canale, raggruppati per obiettivo. Documentazione: docs/HUB.MD, "Eventi
//! della board".
//!
//! - `snapshot`: l'istantanea compatta di una board, ricavata da `BoardView`;
//! - `diff`: gli eventi tra due istantanee (puro);
//! - `route`: l'instradamento degli eventi sui thread (puro);
//! - `texts`: i testi italiani dei messaggi e i titoli dei thread;
//! - `feed`: il thread che riceve le viste pubblicate, tiene le istantanee e
//!   applica gli eventi (`HubFeed`, stato Tauri).

mod diff;
mod feed;
#[cfg(test)]
mod fixtures;
mod route;
mod snapshot;
mod texts;

pub use feed::HubFeed;
