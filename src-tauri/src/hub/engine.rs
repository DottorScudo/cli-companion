//! Il motore dei segretari: fa parlare i bot dell'Agents HUB. Quando l'utente
//! scrive "@mario a che punto siamo?" in un thread, sveglia solo Mario, gli
//! apre una sessione con un contesto piccolo, chiama il modello configurato con
//! i soli tool consentiti a Mario, esegue i tool in Rust e scrive la risposta
//! nel thread (`Author::Bot`). E' un sottosistema del backend, staccato dai
//! coding agents: niente skill, niente MCP. Documentazione: docs/HUB.MD,
//! "Motore dei segretari".
//!
//! Un file per responsabilita':
//! - `chat`: i tipi della conversazione e il trait `ChatModel`;
//! - `openai`: il client reale (`async-openai`) e la classificazione degli errori;
//! - `tools`: schema, descrizione e validazione dei tool, accesso alle board;
//! - `board_query`: filtri, ricerca, ordine e pagine dei task di `board_status`;
//! - `output`: board e thread in JSON compatto per il modello;
//! - `planning`: `open_plan_task` e `decide_proposal` (il task `plan`, la
//!   proposta a cui risponde l'utente, la decisione);
//! - `relay`: `relay_answer` (l'unica domanda in attesa del thread, la consegna);
//! - `context`: system prompt, messaggio di apertura di un turno, riassunto;
//! - `mentions`: le menzioni `@nome`;
//! - `sessions`: sessioni, titolare, code dei thread e scadenze (puro);
//! - `turn`: il ciclo di tool calling, la compattazione e il riassunto;
//! - `texts`: i messaggi di sistema in italiano;
//! - `runtime`: lo stato Tauri `SecretaryEngine`, gli esecutori e lo schedulatore;
//! - `jobs`: i lavori della coda (turno e riassunto) sull'app;
//! - `host`: gli effetti reali su thread, board, portachiavi;
//! - `status`: l'evento `hub-secretary-status` e il comando `hub_secretary_status`.

mod audit;
mod board_query;
mod chat;
mod context;
#[cfg(test)]
mod fake;
#[cfg(test)]
mod fixtures;
mod host;
mod jobs;
mod mentions;
mod openai;
mod output;
mod planning;
mod relay;
mod runtime;
mod sessions;
pub mod status;
mod texts;
mod thread_page;
mod tools;
mod turn;
mod workspace_names;

pub use runtime::SecretaryEngine;
