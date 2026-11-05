//! Il registro dei connettori sostituito nei test, per thread: un test registra i
//! connettori finti e il percorso reale (`registry::all` / `find`) li vede. Ogni test
//! gira nel proprio thread: gli altri, in parallelo, vedono il registro di produzione.
//! La guard ripristina tutto quando esce di scena.

use crate::connectors::contracts::ConnectorImpl;
use std::cell::RefCell;
use std::marker::PhantomData;

thread_local! {
    static OVERRIDE: RefCell<Option<&'static [ConnectorImpl]>> = const { RefCell::new(None) };
}

/// I connettori registrati dal test di questo thread, se ce ne sono.
pub(in crate::connectors) fn registered() -> Option<&'static [ConnectorImpl]> {
    OVERRIDE.with_borrow(|registered| *registered)
}

/// I connettori `list` registrati per il thread corrente e finche' la guard vive.
/// L'elenco resta allocato fino alla fine del processo. Non si sposta fra thread.
#[must_use = "il registro di test vale finche' la guard vive"]
pub struct Registered {
    _same_thread: PhantomData<*const ()>,
}

impl Registered {
    pub fn new(list: Vec<ConnectorImpl>) -> Self {
        let leaked: &'static [ConnectorImpl] = Box::leak(list.into_boxed_slice());
        OVERRIDE.with_borrow_mut(|registered| *registered = Some(leaked));
        Self { _same_thread: PhantomData }
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        OVERRIDE.with_borrow_mut(|registered| *registered = None);
    }
}
