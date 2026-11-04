//! Registro e stato delle integrazioni sostituiti nei test, per thread: con
//! `SecondIntegration` un test registra un'integrazione finta dopo quelle di
//! produzione e la abilita, cosi' il percorso reale (board, scheduler, avvio
//! dei worker, preset, impostazioni) sceglie davvero fra due agenti. Ogni test
//! gira nel proprio thread: gli altri, in parallelo, vedono il registro di
//! produzione. La guard ripristina tutto quando esce di scena.

use crate::agents::contracts::AgentIntegration;
use crate::agents::enablement::{set_enabled, IntegrationsState};
use crate::agents::testing::FakeIntegration;
use std::cell::RefCell;
use std::marker::PhantomData;

struct Override {
    registered: &'static [&'static dyn AgentIntegration],
    state: IntegrationsState,
}

thread_local! {
    static OVERRIDE: RefCell<Option<Override>> = const { RefCell::new(None) };
}

/// Le integrazioni registrate dal test di questo thread, se ce ne sono.
pub(in crate::agents) fn registered() -> Option<&'static [&'static dyn AgentIntegration]> {
    OVERRIDE.with_borrow(|o| o.as_ref().map(|o| o.registered))
}

/// Lo stato delle integrazioni del test di questo thread, se ce n'e' uno.
pub(in crate::agents) fn state() -> Option<IntegrationsState> {
    OVERRIDE.with_borrow(|o| o.as_ref().map(|o| o.state.clone()))
}

/// Un'integrazione finta registrata come seconda e abilitata, per il thread
/// corrente e finche' la guard vive. Il predefinito resta la prima di
/// produzione. Non si sposta fra thread (il registro sostituito e' del thread).
#[must_use = "il registro di test vale finche' la guard vive"]
pub struct SecondIntegration {
    id: &'static str,
    _same_thread: PhantomData<*const ()>,
}

impl SecondIntegration {
    /// Registra `fake` dopo le integrazioni di produzione e abilita tutte.
    /// L'integrazione e l'elenco restano allocati fino alla fine del processo
    /// (il registro presta riferimenti `'static`): pochi byte per test.
    pub fn enabled(fake: FakeIntegration) -> Self {
        assert!(registered().is_none(), "un solo registro di test per thread");
        let id = fake.id;
        let production = super::registry::all();
        assert!(production.iter().all(|i| i.id() != id), "l'id finto {id} e' gia' registrato");
        let fake: &'static FakeIntegration = Box::leak(Box::new(fake));
        let registered: &'static [&'static dyn AgentIntegration] =
            Box::leak(production.iter().copied().chain([fake as &dyn AgentIntegration]).collect());
        let state = IntegrationsState {
            enabled: registered.iter().map(|i| i.id().to_string()).collect(),
            default_agent: production[0].id().to_string(),
        };
        OVERRIDE.set(Some(Override { registered, state }));
        Self { id, _same_thread: PhantomData }
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Disabilita l'integrazione finta, con le regole dell'app.
    pub fn disable(&self) {
        OVERRIDE.with_borrow_mut(|o| {
            let o = o.as_mut().expect("registro di test installato dalla guard");
            o.state = set_enabled(&o.state, o.registered, self.id, false).expect("la finta non e' il predefinito");
        });
    }
}

impl Drop for SecondIntegration {
    fn drop(&mut self) {
        OVERRIDE.set(None);
    }
}

mod tests {
    use super::*;
    use crate::agents::registry;

    const FAKE_ID: &str = "fake-second";

    #[test]
    fn the_guard_registers_and_enables_the_fake_only_while_it_lives() {
        let production: Vec<&str> = registry::all().iter().map(|i| i.id()).collect();
        let default_id = registry::default_agent().id();
        {
            let second = SecondIntegration::enabled(FakeIntegration::full(FAKE_ID));
            let ids: Vec<&str> = registry::all().iter().map(|i| i.id()).collect();
            assert_eq!(ids, [production.as_slice(), &[FAKE_ID]].concat());
            assert!(registry::find(FAKE_ID).is_ok() && registry::is_enabled(FAKE_ID));
            assert_eq!(registry::enabled().len(), production.len() + 1);
            assert_eq!(registry::default_agent().id(), default_id, "il predefinito non cambia");

            second.disable();
            assert!(!registry::is_enabled(FAKE_ID));
            assert!(registry::find(FAKE_ID).is_ok(), "registrata anche se disabilitata");
        }
        assert!(registry::find(FAKE_ID).is_err());
        assert_eq!(registry::all().iter().map(|i| i.id()).collect::<Vec<_>>(), production);
    }

    #[test]
    fn other_threads_keep_the_production_registry() {
        let _second = SecondIntegration::enabled(FakeIntegration::full(FAKE_ID));
        let elsewhere = std::thread::spawn(|| registry::find(FAKE_ID).is_ok()).join().unwrap();
        assert!(!elsewhere);
        assert!(registry::find(FAKE_ID).is_ok());
    }
}
