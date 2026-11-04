//! Canale tra l'app e gli agenti: notifiche degli hook (fine lavoro, richiesta
//! di risposta, permessi) e chiamate RPC delle sessioni automatiche. Il modulo
//! non sa nulla di un agente in particolare: il formato dello stdin degli hook
//! e della decisione di blocco e' il contratto `HookEvents` dell'integrazione
//! (vedi `agents/`), e cosa farne lo decide il core tramite `IpcHandler`.
//!
//! Gli hook degli agenti invocano questo stesso eseguibile:
//! `cli-companion --notify <evento> <agente>` (il JSON dell'hook arriva su
//! stdin, l'id del tab nella variabile `CLI_COMPANION_SESSION`, il segreto di quel
//! tab in `CLI_COMPANION_TOKEN` e la porta dell'app in `CLI_COMPANION_PORT`).
//! La modalita' --notify spedisce una riga JSON su TCP 127.0.0.1 all'istanza
//! GUI, che emette l'evento Tauri `agent-event` verso il frontend.
//!
//! Il listener risponde a ogni notifica con una riga JSON (`{}` oppure
//! `{"block":"<motivo>"}`), scritta dopo che il core ha elaborato l'hook. Solo
//! `--notify finished` (con il segreto) la legge, con un timeout breve: se c'e'
//! `block` stampa su stdout la riga che l'agente si aspetta (la scrive
//! `HookEvents::stop_block_output`) e l'agente prosegue il turno. I client
//! vecchi chiudono senza leggere: scrivere su un socket chiuso e' innocuo. Su
//! stdout, `--notify` non scrive nient'altro.
//!
//! Sullo stesso socket passano anche le chiamate RPC delle sessioni
//! automatiche: una riga JSON con la chiave `rpc`, risposta di una riga. Il
//! server MCP (`cli-companion --mcp`) le spedisce con `rpc_call`. Gli eventi
//! senza l'id del tab e il segreto di QUEL tab (il core li verifica insieme con
//! `IpcHandler::authenticate`) vengono scartati (non arrivano al core ne' al
//! frontend) e il segreto non arriva mai al frontend.
//!
//! Porta: il listener prova `DEFAULT_PORT`; se e' occupata (altra istanza) ne
//! sceglie una libera e i terminali la ricevono in `CLI_COMPANION_PORT`. Le
//! connessioni contemporanee sono limitate. Lo stato e' in `ipc_status`.
//!
//! Struttura: `wire` (protocollo), `client` (`--notify` e RPC), `gate` (quali
//! eventi ammettere, puro), `server` (listener e smistamento).
//!
//! Nota WSL: l'exe gira sempre sull'host Windows (interop), quindi il TCP
//! resta locale e non serve alcun networking cross-VM. Solo std + serde.

mod client;
mod gate;
mod server;
mod wire;

pub use client::{app_port, rpc_call, run_notify};
pub use server::*;
pub use wire::{is_valid_tab_id, NotifyPayload, RpcRequest, RpcResponse, PORT_ENV, SESSION_ENV, TOKEN_ENV};
