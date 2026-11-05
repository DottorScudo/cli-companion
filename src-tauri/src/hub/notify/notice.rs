//! I DTO delle notifiche verso il frontend.

use crate::hub::channel::Channel;
use crate::hub::text::valid_ref_id;
use serde::Serialize;
use ts_rs::TS;

/// Evento Tauri con una notifica pronta: il frontend decide se mostrarla.
pub const HUB_NOTIFY_EVENT: &str = "hub-notify";

/// Il suono della notifica: ne suona uno solo, quando la notifica e' mostrata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "HubNoticeSound")]
pub enum HubNoticeSound {
    /// Messaggio normale.
    Ding,
    /// Serve l'attenzione dell'utente.
    Attention,
}

/// Una notifica per un thread. Mai id interni nei testi: `channel` e
/// `thread_id` servono solo ad aprire il thread al click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "HubNotice")]
pub struct HubNotice {
    #[ts(type = "string")]
    pub channel: Channel,
    pub thread_id: String,
    pub title: String,
    /// Al piu' due righe: "canale · thread" e, se c'e', il messaggio.
    pub body: String,
    pub sound: HubNoticeSound,
}

/// Il thread da aprire al click su una toast; payload di
/// `hub-notification-activated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "HubThreadRef")]
pub struct HubThreadRef {
    pub channel: String,
    pub thread_id: String,
}

/// Valida canale e id del thread ricevuti dal frontend, come i comandi dei thread.
pub fn validate_thread_ref(channel: &str, thread_id: &str) -> Result<HubThreadRef, String> {
    Channel::parse(channel)?;
    if !valid_ref_id(thread_id) {
        return Err("id thread non valido".to_string());
    }
    Ok(HubThreadRef { channel: channel.to_string(), thread_id: thread_id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_reference_is_kept() {
        let r = validate_thread_ref("ws-1", "thr-10-2").unwrap();
        assert_eq!((r.channel.as_str(), r.thread_id.as_str()), ("ws-1", "thr-10-2"));
        assert!(validate_thread_ref("general", "thr-1").is_ok());
    }

    #[test]
    fn unsafe_references_are_rejected() {
        assert!(validate_thread_ref("../x", "thr-1").is_err());
        assert!(validate_thread_ref("", "thr-1").is_err());
        for bad in ["", "a/b", "a b", "thr\n1", &"a".repeat(65)] {
            assert!(validate_thread_ref("general", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_payload_uses_snake_case_names() {
        let notice = HubNotice {
            channel: Channel::General,
            thread_id: "thr-1".into(),
            title: "t".into(),
            body: "b".into(),
            sound: HubNoticeSound::Attention,
        };
        let json = serde_json::to_value(&notice).unwrap();
        assert_eq!(json["thread_id"], "thr-1");
        assert_eq!(json["channel"], "general");
        assert_eq!(json["sound"], "attention");
        assert_eq!(serde_json::to_value(HubNoticeSound::Ding).unwrap(), "ding");
    }
}
