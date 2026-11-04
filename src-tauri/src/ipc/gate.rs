//! Cancello delle notifiche: decide quali eventi ammettere e come riscriverli
//! prima che vadano all'handler e al frontend. Logica PURA: niente `AppHandle`,
//! niente globali, il tempo entra come argomento.

use super::wire::{is_valid_tab_id, NotifyPayload, EVENT_FINISHED, EVENT_NEEDS_INPUT, EVENT_PERMISSION_REQUEST};
use crate::agents::WaitKind;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Dopo un `permission-request` il `needs-input` di tipo permesso per lo
/// stesso tab (puo' arrivare dopo alcuni secondi) e' un duplicato.
const PERMISSION_DEDUPE: Duration = Duration::from_secs(60);
/// Tab ricordati al massimo: oltre, la memoria si svuota.
const MAX_TRACKED_TABS: usize = 256;

/// Stato del cancello: i tab con un `permission-request` recente.
#[derive(Default)]
pub(super) struct NotifyGate {
    recent_permissions: HashMap<String, Instant>,
}

impl NotifyGate {
    /// `None` = scartato. `Some` = ammesso, con il segreto tolto (al frontend
    /// non deve arrivare) e `permission-request` riscritto in `needs-input`.
    /// Si ammette solo un evento con un tab valido e il segreto di QUEL tab
    /// (`authentic(tab, segreto)`): senza, non e' di un figlio del terminale
    /// del tab e non va ne' all'handler ne' al frontend.
    pub(super) fn admit(
        &mut self,
        mut payload: NotifyPayload,
        authentic: impl FnOnce(&str, &str) -> bool,
        now: Instant,
    ) -> Option<NotifyPayload> {
        let proven = match (payload.tab_id.as_deref(), payload.token.as_deref()) {
            (Some(tab), Some(secret)) => is_valid_tab_id(tab) && !secret.is_empty() && authentic(tab, secret),
            _ => false,
        };
        if !proven {
            return None;
        }
        payload.token = None;
        if payload.event == EVENT_PERMISSION_REQUEST {
            // Per il resto dell'app e' un `needs-input` con il dettaglio.
            payload.event = EVENT_NEEDS_INPUT.to_string();
            payload.set_wait_kind(Some(WaitKind::Permission));
            if let Some(tab) = payload.tab_id.as_deref() {
                self.note(tab, now);
            }
        } else if payload.event == EVENT_NEEDS_INPUT
            && payload.wait_kind() == Some(WaitKind::Permission)
            && payload.tab_id.as_deref().is_some_and(|tab| self.seen(tab, now))
        {
            // Gia' segnalato con il dettaglio da `permission-request`.
            return None;
        }
        Some(payload)
    }

    fn note(&mut self, tab: &str, now: Instant) {
        self.recent_permissions.retain(|_, at| now.saturating_duration_since(*at) < PERMISSION_DEDUPE);
        if self.recent_permissions.len() >= MAX_TRACKED_TABS {
            self.recent_permissions.clear();
        }
        self.recent_permissions.insert(tab.to_string(), now);
    }

    fn seen(&self, tab: &str, now: Instant) -> bool {
        self.recent_permissions.get(tab).is_some_and(|at| now.saturating_duration_since(*at) < PERMISSION_DEDUPE)
    }
}

/// `finished` che non e' una vera fine: lo stop e' stato bloccato (il worker
/// continua) o la sessione aspetta lavoro in background e riprendera' da
/// sola. Al frontend non va, altrimenti ding e "finito" sarebbero falsi.
fn is_paused_stop(payload: &NotifyPayload, blocked: bool) -> bool {
    payload.event == EVENT_FINISHED && (blocked || payload.background_work.unwrap_or(0) > 0)
}

/// L'evento ammesso va emesso verso il frontend?
pub(super) fn forwards_to_frontend(payload: &NotifyPayload, blocked: bool) -> bool {
    !is_paused_stop(payload, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Segreti dei tab noti ai test: ognuno vale solo per il suo tab.
    const TOKEN: &str = "secret";
    const TOKEN_T2: &str = "secret-2";

    fn authentic(tab: &str, secret: &str) -> bool {
        match tab {
            "t1" => secret == TOKEN,
            "t2" => secret == TOKEN_T2,
            _ => false,
        }
    }

    fn token_of(tab: &str) -> Option<&'static str> {
        match tab {
            "t1" => Some(TOKEN),
            "t2" => Some(TOKEN_T2),
            _ => None,
        }
    }

    fn payload(event: &str, tab: Option<&str>, token: Option<&str>) -> NotifyPayload {
        NotifyPayload {
            event: event.to_string(),
            agent: "fake".to_string(),
            tab_id: tab.map(str::to_string),
            agent_session_id: None,
            notification_type: None,
            message: None,
            token: token.map(str::to_string),
            stop_hook_active: None,
            background_work: None,
            source: None,
            reason: None,
            transcript_path: None,
            tool_name: None,
        }
    }

    /// Evento del tab `tab` con il suo segreto.
    fn from_tab(event: &str, tab: &str) -> NotifyPayload {
        payload(event, Some(tab), token_of(tab))
    }

    fn prompt(tab: &str) -> NotifyPayload {
        let mut p = from_tab("needs-input", tab);
        p.set_wait_kind(Some(WaitKind::Permission));
        p
    }

    #[test]
    fn events_without_a_valid_secret_are_dropped() {
        let mut gate = NotifyGate::default();
        let now = Instant::now();
        for token in [None, Some(""), Some("wrong"), Some("SECRET"), Some("secret ")] {
            assert_eq!(gate.admit(payload("finished", Some("t1"), token), authentic, now), None, "{token:?}");
        }
        // Un segreto vuoto non arriva al verificatore, anche se ammettesse tutto.
        assert_eq!(gate.admit(payload("finished", Some("t1"), Some("")), |_, _| true, now), None);
        assert_eq!(gate.admit(payload("finished", Some("t1"), None), |_, _| true, now), None);
        // Un evento scartato non lascia traccia nel cancello.
        let mut rejected = payload("permission-request", Some("t1"), Some("wrong"));
        rejected.tool_name = Some("tool".into());
        assert_eq!(gate.admit(rejected, authentic, now), None);
        assert_eq!(gate.admit(prompt("t1"), authentic, now).map(|p| p.event), Some("needs-input".into()));
    }

    #[test]
    fn the_secret_of_another_tab_is_dropped() {
        let mut gate = NotifyGate::default();
        let now = Instant::now();
        // Segreto giusto, ma di un altro tab; tab sconosciuto; id non valido.
        for (tab, token) in [("t1", TOKEN_T2), ("t2", TOKEN), ("t3", TOKEN), ("a b", TOKEN), ("", TOKEN)] {
            let p = payload("finished", Some(tab), Some(token));
            assert_eq!(gate.admit(p, authentic, now), None, "{tab:?}");
        }
        // Un id non valido non arriva nemmeno al verificatore.
        let p = payload("finished", Some("a b"), Some(TOKEN));
        assert_eq!(gate.admit(p, |_, _| true, now), None);
    }

    #[test]
    fn events_without_a_tab_are_dropped() {
        let mut gate = NotifyGate::default();
        let now = Instant::now();
        assert_eq!(gate.admit(payload("finished", None, Some(TOKEN)), |_, _| true, now), None);
        assert_eq!(gate.admit(payload("permission-request", None, Some(TOKEN)), |_, _| true, now), None);
        assert!(gate.recent_permissions.is_empty());
    }

    #[test]
    fn the_verifier_receives_the_tab_and_the_secret_of_the_event() {
        let mut gate = NotifyGate::default();
        let mut seen = Vec::new();
        let out = gate.admit(
            payload("finished", Some("t2"), Some(TOKEN_T2)),
            |tab, secret| {
                seen.push((tab.to_string(), secret.to_string()));
                true
            },
            Instant::now(),
        );
        assert!(out.is_some());
        assert_eq!(seen, vec![("t2".to_string(), TOKEN_T2.to_string())]);
    }

    #[test]
    fn admitted_events_lose_the_token_and_keep_the_rest() {
        let mut gate = NotifyGate::default();
        let mut p = from_tab("finished", "t1");
        p.message = Some("m".into());
        p.background_work = Some(0);
        let out = gate.admit(p.clone(), authentic, Instant::now()).unwrap();
        assert_eq!(out.token, None);
        assert_eq!(NotifyPayload { token: None, ..p }, out);
    }

    #[test]
    fn permission_request_becomes_a_permission_wait() {
        let mut gate = NotifyGate::default();
        let mut p = from_tab("permission-request", "t1");
        p.message = Some("Permesso richiesto".into());
        p.tool_name = Some("tool".into());
        let out = gate.admit(p, authentic, Instant::now()).unwrap();
        assert_eq!(out.event, "needs-input");
        assert_eq!(out.wait_kind(), Some(WaitKind::Permission));
        assert_eq!((out.message.as_deref(), out.tool_name.as_deref()), (Some("Permesso richiesto"), Some("tool")));
        assert_eq!(out.token, None);
    }

    #[test]
    fn duplicate_permission_wait_is_dropped_within_the_window_only() {
        let mut gate = NotifyGate::default();
        let t0 = Instant::now();
        // Nessun permission-request prima: il needs-input passa.
        assert!(gate.admit(prompt("t1"), authentic, t0).is_some());
        gate.admit(from_tab("permission-request", "t1"), authentic, t0).unwrap();
        assert_eq!(gate.admit(prompt("t1"), authentic, t0 + Duration::from_secs(6)), None);
        assert_eq!(gate.admit(prompt("t1"), authentic, t0 + Duration::from_secs(59)), None);
        // Scaduta la finestra: e' una richiesta nuova.
        assert!(gate.admit(prompt("t1"), authentic, t0 + PERMISSION_DEDUPE).is_some());
    }

    #[test]
    fn dedupe_is_per_tab_and_only_for_permission_waits() {
        let mut gate = NotifyGate::default();
        let t0 = Instant::now();
        gate.admit(from_tab("permission-request", "t1"), authentic, t0).unwrap();
        // Altro tab, altro tipo di notifica, altro evento: passano.
        assert!(gate.admit(prompt("t2"), authentic, t0).is_some());
        let mut idle = from_tab("needs-input", "t1");
        idle.set_wait_kind(Some(WaitKind::Idle));
        assert!(gate.admit(idle, authentic, t0).is_some());
        // Il nome grezzo di un agente non e' il tipo neutro del protocollo.
        let mut raw = from_tab("needs-input", "t1");
        raw.notification_type = Some("permission_request".into());
        assert!(gate.admit(raw, authentic, t0).is_some());
        assert!(gate.admit(from_tab("needs-input", "t1"), authentic, t0).is_some());
        assert!(gate.admit(from_tab("finished", "t1"), authentic, t0).is_some());
    }

    #[test]
    fn tracked_tabs_are_bounded() {
        let mut gate = NotifyGate::default();
        let t0 = Instant::now();
        for i in 0..MAX_TRACKED_TABS {
            gate.note(&format!("t{i}"), t0);
        }
        assert_eq!(gate.recent_permissions.len(), MAX_TRACKED_TABS);
        // Pieno e nessuno scaduto: si svuota e riparte dal nuovo.
        gate.note("new", t0);
        assert_eq!(gate.recent_permissions.len(), 1);
        assert!(gate.seen("new", t0) && !gate.seen("t0", t0));
        // Pieno ma con voci scadute: si scartano solo quelle.
        let mut gate = NotifyGate::default();
        for i in 0..MAX_TRACKED_TABS {
            gate.note(&format!("t{i}"), t0);
        }
        let later = t0 + PERMISSION_DEDUPE;
        gate.note("new", later);
        assert_eq!(gate.recent_permissions.len(), 1);
        // La rimozione dei vecchi lascia i recenti.
        gate.note("t9", later + Duration::from_secs(1));
        assert!(gate.seen("new", later + Duration::from_secs(1)) && gate.seen("t9", later + Duration::from_secs(1)));
    }

    #[test]
    fn paused_stop_is_not_forwarded_to_frontend() {
        let mut p = payload("finished", Some("t"), None);
        assert!(forwards_to_frontend(&p, false));
        assert!(!forwards_to_frontend(&p, true));
        p.background_work = Some(0);
        assert!(forwards_to_frontend(&p, false));
        p.background_work = Some(2);
        assert!(!forwards_to_frontend(&p, false));
        // Solo `finished`: gli altri eventi passano sempre.
        let mut n = payload("needs-input", Some("t"), None);
        n.background_work = Some(2);
        assert!(forwards_to_frontend(&n, true));
    }
}
