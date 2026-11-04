//! Cancello delle notifiche: decide quali eventi ammettere e come riscriverli
//! prima che vadano all'handler e al frontend. Logica PURA: niente `AppHandle`,
//! niente globali, il tempo entra come argomento.

use super::wire::{NotifyPayload, EVENT_FINISHED, EVENT_NEEDS_INPUT, EVENT_PERMISSION_REQUEST};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tipo di notifica neutro del protocollo per "aspetta un permesso": lo usano
/// sia la riscrittura di `permission-request` sia lo scarto del duplicato che
/// un agente puo' mandare come `needs-input` poco dopo.
pub(super) const PERMISSION_PROMPT: &str = "permission_prompt";
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
    /// `None` = scartato. `Some` = ammesso, con il token tolto (al frontend
    /// non deve arrivare) e `permission-request` riscritto in `needs-input`.
    /// Un evento senza il token giusto non e' di un figlio di un terminale
    /// dell'app: non va ne' all'handler ne' al frontend.
    pub(super) fn admit(
        &mut self,
        mut payload: NotifyPayload,
        expected_token: &str,
        now: Instant,
    ) -> Option<NotifyPayload> {
        let token_ok = !expected_token.is_empty() && payload.token.as_deref() == Some(expected_token);
        if !token_ok {
            return None;
        }
        payload.token = None;
        if payload.event == EVENT_PERMISSION_REQUEST {
            // Per il resto dell'app e' un `needs-input` con il dettaglio.
            payload.event = EVENT_NEEDS_INPUT.to_string();
            payload.notification_type = Some(PERMISSION_PROMPT.to_string());
            if let Some(tab) = payload.tab_id.as_deref() {
                self.note(tab, now);
            }
        } else if payload.event == EVENT_NEEDS_INPUT
            && payload.notification_type.as_deref() == Some(PERMISSION_PROMPT)
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

    const TOKEN: &str = "secret";

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

    fn prompt(tab: Option<&str>) -> NotifyPayload {
        let mut p = payload("needs-input", tab, Some(TOKEN));
        p.notification_type = Some(PERMISSION_PROMPT.to_string());
        p
    }

    #[test]
    fn events_without_a_valid_token_are_dropped() {
        let mut gate = NotifyGate::default();
        let now = Instant::now();
        for token in [None, Some(""), Some("wrong"), Some("SECRET"), Some("secret ")] {
            assert_eq!(gate.admit(payload("finished", Some("t1"), token), TOKEN, now), None, "{token:?}");
        }
        // Token atteso vuoto: nessun token e' valido, nemmeno uno vuoto.
        assert_eq!(gate.admit(payload("finished", Some("t1"), Some("")), "", now), None);
        assert_eq!(gate.admit(payload("finished", Some("t1"), None), "", now), None);
        // Un evento scartato non lascia traccia nel cancello.
        let mut rejected = payload("permission-request", Some("t1"), Some("wrong"));
        rejected.tool_name = Some("tool".into());
        assert_eq!(gate.admit(rejected, TOKEN, now), None);
        assert_eq!(gate.admit(prompt(Some("t1")), TOKEN, now).map(|p| p.event), Some("needs-input".into()));
    }

    #[test]
    fn admitted_events_lose_the_token_and_keep_the_rest() {
        let mut gate = NotifyGate::default();
        let mut p = payload("finished", Some("t1"), Some(TOKEN));
        p.message = Some("m".into());
        p.background_work = Some(0);
        let out = gate.admit(p.clone(), TOKEN, Instant::now()).unwrap();
        assert_eq!(out.token, None);
        assert_eq!(NotifyPayload { token: None, ..p }, out);
    }

    #[test]
    fn permission_request_becomes_a_permission_prompt() {
        let mut gate = NotifyGate::default();
        let mut p = payload("permission-request", Some("t1"), Some(TOKEN));
        p.message = Some("Permesso richiesto".into());
        p.tool_name = Some("tool".into());
        let out = gate.admit(p, TOKEN, Instant::now()).unwrap();
        assert_eq!(out.event, "needs-input");
        assert_eq!(out.notification_type.as_deref(), Some(PERMISSION_PROMPT));
        assert_eq!((out.message.as_deref(), out.tool_name.as_deref()), (Some("Permesso richiesto"), Some("tool")));
        assert_eq!(out.token, None);
    }

    #[test]
    fn duplicate_permission_prompt_is_dropped_within_the_window_only() {
        let mut gate = NotifyGate::default();
        let t0 = Instant::now();
        // Nessun permission-request prima: il needs-input passa.
        assert!(gate.admit(prompt(Some("t1")), TOKEN, t0).is_some());
        gate.admit(payload("permission-request", Some("t1"), Some(TOKEN)), TOKEN, t0).unwrap();
        assert_eq!(gate.admit(prompt(Some("t1")), TOKEN, t0 + Duration::from_secs(6)), None);
        assert_eq!(gate.admit(prompt(Some("t1")), TOKEN, t0 + Duration::from_secs(59)), None);
        // Scaduta la finestra: e' una richiesta nuova.
        assert!(gate.admit(prompt(Some("t1")), TOKEN, t0 + PERMISSION_DEDUPE).is_some());
    }

    #[test]
    fn dedupe_is_per_tab_and_only_for_permission_prompts() {
        let mut gate = NotifyGate::default();
        let t0 = Instant::now();
        gate.admit(payload("permission-request", Some("t1"), Some(TOKEN)), TOKEN, t0).unwrap();
        // Altro tab, tab assente, altro tipo di notifica, altro evento: passano.
        assert!(gate.admit(prompt(Some("t2")), TOKEN, t0).is_some());
        assert!(gate.admit(prompt(None), TOKEN, t0).is_some());
        let mut idle = payload("needs-input", Some("t1"), Some(TOKEN));
        idle.notification_type = Some("idle".into());
        assert!(gate.admit(idle, TOKEN, t0).is_some());
        assert!(gate.admit(payload("needs-input", Some("t1"), Some(TOKEN)), TOKEN, t0).is_some());
        assert!(gate.admit(payload("finished", Some("t1"), Some(TOKEN)), TOKEN, t0).is_some());
        // Un permission-request senza tab non ricorda nulla.
        let mut fresh = NotifyGate::default();
        fresh.admit(payload("permission-request", None, Some(TOKEN)), TOKEN, t0).unwrap();
        assert!(fresh.recent_permissions.is_empty());
        assert!(fresh.admit(prompt(None), TOKEN, t0).is_some());
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
