//! La parte che esegue dei connettori:
//! - `http`: il client HTTP del contesto (allowlist degli host);
//! - `backoff`: l'attesa fra i tentativi e il parser di `Retry-After` (puro);
//! - `health`: gli stati di salute e l'evento `connector-status`;
//! - `store`: `connectors.json` in memoria e lo stato delle istanze;
//! - `time`: l'attesa e i timeout, iniettabili;
//! - `round`: un giro di una fonte (poll, consegna, dedupe, cursore);
//! - `pairing`, `contact`, `contact_loop`: i canali di contatto (abbinamento,
//!   messaggi in entrata, notifiche in uscita);
//! - `notices`: dalle notifiche dell'HUB ai supervisori di contatto;
//! - `supervisor`: il task di un'istanza (ambiente, ritmo degli errori, ciclo delle fonti);
//! - `migrate`: migrazione e validazione della configurazione di un'istanza;
//! - `manager`: `ConnectorsRuntime`, lo stato gestito che avvia e ferma i supervisori;
//! - `hub_port`: la porta verso l'HUB vera.

pub mod backoff;
pub mod contact;
pub mod contact_loop;
pub mod health;
pub mod http;
pub mod hub_port;
pub mod manager;
pub mod migrate;
pub mod notices;
pub mod pairing;
pub mod round;
pub mod store;
pub mod supervisor;
pub mod time;
