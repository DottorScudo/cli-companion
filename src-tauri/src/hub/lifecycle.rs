//! Chi vuole sapere quando l'HUB si accende o si spegne, quando un workspace
//! sparisce e quando un thread viene eliminato (un connettore che deve fermare i
//! suoi supervisori, togliere le regole di un canale o eliminare le sotto-chat). `hub` non conosce chi ascolta: `lib.rs` registra gli
//! ascoltatori (`set_lifecycle_listener`), come fa con i destinatari delle
//! notifiche. Gli ascoltatori girano sul thread di chi cambia lo stato (in
//! genere uno del pool bloccante), mai con un lock dell'HUB preso: devono essere
//! rapidi e non andare in panico.

use super::channel::Channel;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

/// Un ascoltatore del ciclo di vita dell'HUB.
pub trait HubLifecycle: Send + Sync {
    /// L'HUB e' stato acceso o spento (`enabled` e' il nuovo valore). Chiamato
    /// solo sui passaggi effettivi, dopo che il nuovo stato e' stato salvato.
    fn hub_enabled_changed(&self, app: &AppHandle, enabled: bool);
    /// Il workspace `ws_id` e' stato rimosso (anche con l'HUB spento).
    fn workspace_removed(&self, app: &AppHandle, ws_id: &str);
    /// Il thread `thread_id` di `channel` e' stato eliminato a mano. Chiamato dopo il
    /// salvataggio del canale. Predefinito: nessuna reazione.
    fn thread_removed(&self, _app: &AppHandle, _channel: &Channel, _thread_id: &str) {}
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
        self.for_each(|listener| listener.workspace_removed(app, ws_id));
    }

    /// Avvisa gli ascoltatori che il thread `thread_id` di `channel` e' stato eliminato.
    pub fn thread_removed(&self, app: &AppHandle, channel: &Channel, thread_id: &str) {
        self.for_each(|listener| listener.thread_removed(app, channel, thread_id));
    }

    fn for_each_transition(&self, was_enabled: bool, enabled: bool, mut f: impl FnMut(&dyn HubLifecycle, bool)) {
        let Some(now) = transition(was_enabled, enabled) else { return };
        self.for_each(|listener| f(listener, now));
    }

    fn for_each(&self, mut f: impl FnMut(&dyn HubLifecycle)) {
        for listener in self.snapshot() {
            f(listener.as_ref());
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

    #[test]
    fn every_listener_is_visited_and_thread_removed_is_optional() {
        // `Noop` non implementa `thread_removed`: il predefinito non fa nulla.
        let listeners = HubLifecycleListeners::default();
        listeners.add(Arc::new(Noop));
        listeners.add(Arc::new(Noop));
        let mut visited = 0;
        listeners.for_each(|_| visited += 1);
        assert_eq!(visited, 2);
    }
}
