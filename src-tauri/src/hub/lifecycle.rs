//! Chi vuole sapere quando l'HUB si accende o si spegne e quando un workspace
//! sparisce (un connettore che deve fermare i suoi supervisori o togliere le
//! regole di un canale). `hub` non conosce chi ascolta: `lib.rs` registra gli
//! ascoltatori (`set_lifecycle_listener`), come fa con i destinatari delle
//! notifiche. Gli ascoltatori girano sul thread di chi cambia lo stato (in
//! genere uno del pool bloccante), mai con un lock dell'HUB preso: devono essere
//! rapidi e non andare in panico.

use std::sync::{Arc, Mutex};
use tauri::AppHandle;

/// Un ascoltatore del ciclo di vita dell'HUB.
pub trait HubLifecycle: Send + Sync {
    /// L'HUB e' stato acceso o spento (`enabled` e' il nuovo valore). Chiamato
    /// solo sui passaggi effettivi, dopo che il nuovo stato e' stato salvato.
    fn hub_enabled_changed(&self, app: &AppHandle, enabled: bool);
    /// Il workspace `ws_id` e' stato rimosso (anche con l'HUB spento).
    fn workspace_removed(&self, app: &AppHandle, ws_id: &str);
}

/// Gli ascoltatori registrati. Stato gestito da Tauri.
#[derive(Default)]
pub struct HubLifecycleListeners {
    listeners: Mutex<Vec<Arc<dyn HubLifecycle>>>,
}

/// Il nuovo valore dell'interruttore se e' cambiato davvero, altrimenti `None`.
fn transition(was_enabled: bool, enabled: bool) -> Option<bool> {
    (was_enabled != enabled).then_some(enabled)
}

impl HubLifecycleListeners {
    /// Aggiunge un ascoltatore; vale finche' l'app gira.
    pub fn add(&self, listener: Arc<dyn HubLifecycle>) {
        self.lock().push(listener);
    }

    /// L'interruttore dell'HUB era `was_enabled` ed ora e' `enabled`: avvisa gli
    /// ascoltatori solo se e' cambiato.
    pub fn enabled_changed(&self, app: &AppHandle, was_enabled: bool, enabled: bool) {
        self.for_each_transition(was_enabled, enabled, |listener, now| listener.hub_enabled_changed(app, now));
    }

    /// Avvisa gli ascoltatori che il workspace `ws_id` e' stato rimosso.
    pub fn workspace_removed(&self, app: &AppHandle, ws_id: &str) {
        for listener in self.snapshot() {
            listener.workspace_removed(app, ws_id);
        }
    }

    fn for_each_transition(&self, was_enabled: bool, enabled: bool, mut f: impl FnMut(&dyn HubLifecycle, bool)) {
        let Some(now) = transition(was_enabled, enabled) else { return };
        for listener in self.snapshot() {
            f(listener.as_ref(), now);
        }
    }

    /// Copia dell'elenco: gli ascoltatori girano senza il lock preso.
    fn snapshot(&self) -> Vec<Arc<dyn HubLifecycle>> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<dyn HubLifecycle>>> {
        self.listeners.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Noop;

    impl HubLifecycle for Noop {
        fn hub_enabled_changed(&self, _: &AppHandle, _: bool) {}
        fn workspace_removed(&self, _: &AppHandle, _: &str) {}
    }

    #[test]
    fn only_real_transitions_are_reported() {
        assert_eq!(transition(false, true), Some(true));
        assert_eq!(transition(true, false), Some(false));
        assert_eq!(transition(true, true), None);
        assert_eq!(transition(false, false), None);
    }

    #[test]
    fn listeners_are_called_in_registration_order_only_on_transitions() {
        let listeners = HubLifecycleListeners::default();
        listeners.add(Arc::new(Noop));
        listeners.add(Arc::new(Noop));
        let mut seen = Vec::new();
        listeners.for_each_transition(false, true, |_, now| seen.push(now));
        listeners.for_each_transition(true, true, |_, now| seen.push(now));
        listeners.for_each_transition(true, false, |_, now| seen.push(now));
        // Due ascoltatori, due passaggi veri: il passaggio vuoto non chiama nessuno.
        assert_eq!(seen, vec![true, true, false, false]);
        assert_eq!(listeners.snapshot().len(), 2);
    }
}
