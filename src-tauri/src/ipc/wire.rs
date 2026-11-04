//! Protocollo sul socket: tipi, costanti e (de)serializzazione di una riga.
//! Non sa nulla degli agenti: i dettagli di un evento arrivano gia' normalizzati.

use crate::agents::HookEventKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::time::Duration;

/// Porta preferita del listener locale. Solo 127.0.0.1: nessun prompt del
/// firewall. Se e' occupata il listener ne sceglie una libera e la comunica
/// alle sessioni con `PORT_ENV`.
pub const DEFAULT_PORT: u16 = 17421;
/// Variabile d'ambiente impostata dal pty con l'id del tab: gli hook la
/// ereditano (anche da WSL, via WSLENV) e cosi' l'evento si lega al tab.
pub const SESSION_ENV: &str = "CLI_COMPANION_SESSION";
/// Token casuale dell'app, impostato dal pty accanto a `SESSION_ENV`: chi lo
/// conosce e' un processo figlio di un terminale di cli-companion.
pub const TOKEN_ENV: &str = "CLI_COMPANION_TOKEN";
/// Porta su cui l'app e' in ascolto, impostata dal pty accanto al token.
/// Assente = `DEFAULT_PORT`.
pub const PORT_ENV: &str = "CLI_COMPANION_PORT";

/// Eventi accettati sul socket e in --notify: `test` e' il self-test, gli
/// altri arrivano dagli hook dei coding agent (vedi `agents/`).
pub(super) const KNOWN_EVENTS: [&str; 6] =
    ["test", "session-start", "session-end", "finished", "needs-input", "permission-request"];
pub(super) const EVENT_FINISHED: &str = "finished";
pub(super) const EVENT_NEEDS_INPUT: &str = "needs-input";
/// Evento di richiesta di un permesso: il backend lo converte in `needs-input`
/// con il dettaglio di cosa aspetta il permesso.
pub(super) const EVENT_PERMISSION_REQUEST: &str = "permission-request";

/// Tetto di una riga RPC in ingresso e di una risposta letta dal client.
pub(super) const RPC_MAX_BYTES: u64 = 1024 * 1024;
pub(super) const RPC_RESPONSE_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// Tetto di una notifica sul socket e della sua risposta.
pub(super) const WIRE_MAX_BYTES: u64 = 64 * 1024;
/// Timeout di lettura della riga di richiesta (non della risposta).
pub(super) const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(2);
const TAB_ID_MAX: usize = 64;

/// Payload sul socket e verso il frontend (una riga JSON + `\n`). I nomi dei
/// campi sono quelli del protocollo, condivisi con il frontend e con i client
/// gia' installati: non vanno rinominati.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotifyPayload {
    pub event: String,
    pub agent: String,
    pub tab_id: Option<String>,
    pub agent_session_id: Option<String>,
    pub notification_type: Option<String>,
    pub message: Option<String>,
    /// Token dell'app (da `TOKEN_ENV`). Serve solo al backend: prima di
    /// emettere verso il frontend va azzerato.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Solo `finished`: l'agente sta gia' proseguendo perche' uno stop e' stato
    /// bloccato.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_hook_active: Option<bool>,
    /// Solo `finished`: lavoro in background ancora attivo. `None` = dato assente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_work: Option<u32>,
    /// Solo `session-start`: come e' iniziata la sessione dell'agente
    /// (es. `startup`, `resume`, `clear`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Solo `session-end`: perche' e' finita la sessione dell'agente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Path del transcript della sessione, come lo vede l'agente (Windows o
    /// Linux/WSL): serve al frontend per leggere i consumi di token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    /// Solo `permission-request`: tool che aspetta il permesso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// Risposta del listener a una notifica: `{}` oppure `{"block":"<motivo>"}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotifyReply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

impl NotifyReply {
    /// Riga JSON senza il `\n` finale.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Richiesta RPC sul socket: `{"rpc":1,"token":..,"session":..,"method":..,"params":{..}}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcRequest {
    pub rpc: u32,
    #[serde(default)]
    pub token: String,
    /// Id del tab chiamante.
    #[serde(default)]
    pub session: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Risposta RPC: `{"ok":true,"result":..}` oppure `{"ok":false,"error":".."}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub result: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn success(result: Value) -> Self {
        Self { ok: true, result, error: None }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self { ok: false, result: Value::Null, error: Some(error.into()) }
    }

    /// Esito di un dispatch -> risposta.
    pub fn from_result(r: Result<Value, String>) -> Self {
        match r {
            Ok(v) => Self::success(v),
            Err(e) => Self::failure(e),
        }
    }

    /// Riga JSON senza il `\n` finale.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"risposta non serializzabile"}"#.to_string())
    }
}

/// Una riga letta dal socket: notifica di un hook o chiamata RPC.
#[derive(Debug)]
pub enum Wire {
    Notify(NotifyPayload),
    Rpc(RpcRequest),
}

pub(super) fn is_known_event(event: &str) -> bool {
    KNOWN_EVENTS.contains(&event)
}

/// Tipo di evento hook per il contratto degli agenti. `test` non e' un evento
/// di un agente: nessun tipo, nessun dettaglio da leggere.
pub(super) fn hook_kind(event: &str) -> Option<HookEventKind> {
    Some(match event {
        "session-start" => HookEventKind::SessionStart,
        "finished" => HookEventKind::Finished,
        "session-end" => HookEventKind::SessionEnd,
        "needs-input" => HookEventKind::NeedsInput,
        "permission-request" => HookEventKind::PermissionRequest,
        _ => return None,
    })
}

/// Id tab ammesso: ASCII alfanumerico, `-` e `_`, 1..=64 caratteri. Finisce
/// in una variabile d'ambiente e in WSLENV, quindi niente caratteri strani.
pub fn is_valid_tab_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= TAB_ID_MAX && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub(super) fn build_line(payload: &NotifyPayload) -> Result<String, String> {
    if !is_known_event(&payload.event) {
        return Err(format!("evento sconosciuto: {}", payload.event));
    }
    serde_json::to_string(payload).map_err(|e| e.to_string())
}

pub(super) fn parse_line(line: &str) -> Result<NotifyPayload, String> {
    let payload: NotifyPayload = serde_json::from_str(line.trim()).map_err(|_| "payload non valido".to_string())?;
    if !is_known_event(&payload.event) {
        return Err(format!("evento sconosciuto: {}", payload.event));
    }
    Ok(payload)
}

/// Distingue una riga RPC (chiave `rpc`) da una notifica. Le notifiche
/// restano limitate a 64 KB, le RPC a 1 MB (gia' garantito dalla lettura).
pub(super) fn parse_wire(line: &str) -> Result<Wire, String> {
    let v: Value = serde_json::from_str(line.trim()).map_err(|_| "payload non valido".to_string())?;
    if v.get("rpc").is_some() {
        let req: RpcRequest = serde_json::from_value(v).map_err(|e| format!("richiesta RPC non valida: {e}"))?;
        return Ok(Wire::Rpc(req));
    }
    if line.len() as u64 > WIRE_MAX_BYTES {
        return Err("payload troppo grande".to_string());
    }
    parse_line(line).map(Wire::Notify)
}

/// Legge una riga dal socket (tetto `RPC_MAX_BYTES`) e la classifica. Il
/// timeout di lettura vale solo qui, non per il tempo di elaborazione.
pub(super) fn read_wire(stream: &mut TcpStream) -> Result<Wire, String> {
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT)).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new((&*stream).take(RPC_MAX_BYTES)).read_line(&mut line).map_err(|e| e.to_string())?;
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    parse_wire(&line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(event: &str) -> NotifyPayload {
        NotifyPayload {
            event: event.to_string(),
            agent: "fake".to_string(),
            tab_id: Some("tab-1".to_string()),
            agent_session_id: None,
            notification_type: None,
            message: None,
            token: None,
            stop_hook_active: None,
            background_work: None,
            source: None,
            reason: None,
            transcript_path: None,
            tool_name: None,
        }
    }

    #[test]
    fn payload_roundtrips_and_keeps_the_protocol_field_names() {
        let mut full = sample("finished");
        full.token = Some("t".into());
        full.stop_hook_active = Some(true);
        full.background_work = Some(2);
        full.source = Some("clear".into());
        full.reason = Some("other".into());
        full.transcript_path = Some(r"C:\x\s.jsonl".into());
        full.tool_name = Some("tool".into());
        full.agent_session_id = Some("s".into());
        assert_eq!(parse_line(&build_line(&full).unwrap()).unwrap(), full);
        let v: Value = serde_json::from_str(&build_line(&full).unwrap()).unwrap();
        for key in [
            "event",
            "agent",
            "tab_id",
            "agent_session_id",
            "notification_type",
            "message",
            "token",
            "stop_hook_active",
            "background_work",
            "source",
            "reason",
            "transcript_path",
            "tool_name",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
    }

    #[test]
    fn optional_fields_are_not_serialized_when_absent() {
        let ser = serde_json::to_string(&sample("finished")).unwrap();
        for key in
            ["token", "stop_hook_active", "background_work", "\"source\"", "\"reason\"", "transcript_path", "tool_name"]
        {
            assert!(!ser.contains(key), "{key} in {ser}");
        }
        // Payload di un client vecchio, senza i campi opzionali: valido.
        let old = parse_line(r#"{"event":"finished","agent":"x"}"#).unwrap();
        assert_eq!(old.tab_id, None);
        assert_eq!((old.token, old.stop_hook_active, old.background_work), (None, None, None));
        assert_eq!((old.source, old.reason, old.transcript_path, old.tool_name), (None, None, None, None));
        let with = parse_line(r#"{"event":"finished","agent":"x","token":"abc"}"#).unwrap();
        assert_eq!(with.token.as_deref(), Some("abc"));
    }

    #[test]
    fn wire_rejects_unknown_event_and_garbage() {
        assert!(build_line(&sample("nope")).is_err());
        assert!(parse_line("non json").is_err());
        assert!(parse_line(r#"{"event":"nope","agent":"x"}"#).is_err());
    }

    #[test]
    fn wire_classifies_rpc_and_notify() {
        let rpc = r#"{"rpc":1,"token":"t","session":"tab-1","method":"whoami","params":{}}"#;
        match parse_wire(rpc).unwrap() {
            Wire::Rpc(r) => {
                assert_eq!((r.rpc, r.token.as_str(), r.session.as_str()), (1, "t", "tab-1"));
                assert_eq!(r.method, "whoami");
            }
            Wire::Notify(_) => panic!("attesa una RPC"),
        }
        // Params e token assenti: parsing ok (e' l'auth a rifiutare).
        let Wire::Rpc(r) = parse_wire(r#"{"rpc":1,"method":"x"}"#).unwrap() else {
            panic!("attesa una RPC");
        };
        assert!(r.token.is_empty() && r.params.is_null());
        assert!(matches!(parse_wire(r#"{"event":"finished","agent":"x"}"#).unwrap(), Wire::Notify(_)));
        assert!(parse_wire(r#"{"rpc":1}"#).is_err()); // manca method
        assert!(parse_wire("non json").is_err());
        // Le notifiche hanno il tetto di 64 KB, le RPC no.
        let big = "x".repeat(WIRE_MAX_BYTES as usize);
        let notify = format!(r#"{{"event":"finished","agent":"{big}"}}"#);
        assert!(parse_wire(&notify).is_err());
        let rpc = format!(r#"{{"rpc":1,"method":"m","params":{{"a":"{big}"}}}}"#);
        assert!(matches!(parse_wire(&rpc), Ok(Wire::Rpc(_))));
    }

    #[test]
    fn rpc_response_shapes() {
        let ok = RpcResponse::success(serde_json::json!({"a":1}));
        assert_eq!(ok.to_line(), r#"{"ok":true,"result":{"a":1}}"#);
        let err = RpcResponse::failure("no");
        assert_eq!(err.to_line(), r#"{"ok":false,"error":"no"}"#);
        let back: RpcResponse = serde_json::from_str(&ok.to_line()).unwrap();
        assert_eq!(back, ok);
        assert_eq!(RpcResponse::from_result(Err("x".into())), RpcResponse::failure("x"));
    }

    #[test]
    fn reply_line_shapes() {
        assert_eq!(NotifyReply::default().to_line(), "{}");
        assert_eq!(NotifyReply { block: Some("x".into()) }.to_line(), r#"{"block":"x"}"#);
    }

    #[test]
    fn tab_id_validation() {
        assert!(is_valid_tab_id("3f2b9c1e-aaaa-4bbb-8ccc-0123456789ab"));
        assert!(is_valid_tab_id("tab_1"));
        assert!(!is_valid_tab_id(""));
        assert!(!is_valid_tab_id("a:b"));
        assert!(!is_valid_tab_id("a/b"));
        assert!(!is_valid_tab_id(&"x".repeat(65)));
        assert!(is_valid_tab_id(&"x".repeat(64)));
    }

    #[test]
    fn every_known_event_but_test_maps_to_a_hook_kind() {
        assert_eq!(hook_kind("test"), None);
        assert_eq!(hook_kind("stop"), None);
        for event in KNOWN_EVENTS.into_iter().filter(|e| *e != "test") {
            let kind = hook_kind(event).unwrap_or_else(|| panic!("{event}"));
            // Il nome sul filo coincide con quello serializzato del tipo.
            assert_eq!(serde_json::to_value(kind).unwrap(), event);
        }
        assert_eq!(KNOWN_EVENTS.len() - 1, HookEventKind::ALL.len());
    }
}
