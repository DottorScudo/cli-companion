//! Tabella delle sessioni PTY. Il limite di sessioni, l'unicita' del tab e
//! l'inserimento avvengono sotto lo stesso lock: chi apre una sessione
//! prenota prima lo slot (`Reservation`) e lo conferma a spawn riuscito;
//! una prenotazione non confermata si rilascia da sola.

use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(super) const MAX_SESSIONS: usize = 32;

pub(super) struct PtySession {
    /// Arc: il resize clona il master e lo esegue senza tenere il lock della
    /// mappa (ResizePseudoConsole puo' bloccarsi).
    pub master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    /// Ultima richiesta di resize: i resize girano su thread diversi e uno
    /// vecchio arrivato dopo non deve sovrascrivere la dimensione piu' recente.
    pub resize_seq: Arc<AtomicU64>,
    /// Coda verso il thread di scrittura dedicato: chi scrive accoda soltanto,
    /// non fa mai I/O (una shell che non legge non blocca ne' la UI ne' le
    /// altre sessioni). L'ordine dei byte e' quello di accodamento.
    pub writer: Sender<Vec<u8>>,
    pub child: Mutex<Box<dyn Child + Send + Sync>>,
}

impl PtySession {
    /// Termina la shell e rilascia il master (ClosePseudoConsole). Entrambi
    /// possono bloccarsi: da chiamare fuori dal main thread.
    pub fn shutdown(self) {
        let _ = self.child.lock().unwrap().kill();
    }
}

/// Slot di un tab: prenotato (`session` assente) mentre la shell si avvia,
/// poi occupato dalla sessione viva.
struct Slot {
    tab_id: String,
    workspace_id: String,
    session: Option<PtySession>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReserveError {
    Full,
    DuplicateTab,
    Closing,
}

impl fmt::Display for ReserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReserveError::Full => "troppe sessioni aperte",
            ReserveError::DuplicateTab => "il tab ha gia' una sessione",
            ReserveError::Closing => "app in chiusura",
        })
    }
}

#[derive(Default)]
pub(super) struct Registry {
    slots: HashMap<u32, Slot>,
    /// Indice tab -> sessione: rende univoca e deterministica la ricerca per tab.
    tabs: HashMap<String, u32>,
    last_id: u32,
    closing: bool,
}

impl Registry {
    fn reserve(&mut self, tab_id: &str, workspace_id: &str) -> Result<u32, ReserveError> {
        if self.closing {
            return Err(ReserveError::Closing);
        }
        if self.slots.len() >= MAX_SESSIONS {
            return Err(ReserveError::Full);
        }
        if self.tabs.contains_key(tab_id) {
            return Err(ReserveError::DuplicateTab);
        }
        self.last_id = self.last_id.wrapping_add(1);
        let id = self.last_id;
        self.slots
            .insert(id, Slot { tab_id: tab_id.to_string(), workspace_id: workspace_id.to_string(), session: None });
        self.tabs.insert(tab_id.to_string(), id);
        Ok(id)
    }

    /// Rimuove lo slot (prenotato o vivo) e la sua voce nell'indice dei tab.
    fn release(&mut self, id: u32) -> Option<Slot> {
        let slot = self.slots.remove(&id)?;
        self.tabs.remove(&slot.tab_id);
        Some(slot)
    }

    /// Occupa lo slot prenotato `id`. A chiusura dell'app in corso restituisce
    /// la sessione al chiamante, che deve terminarla.
    fn commit(&mut self, id: u32, session: PtySession) -> Result<(), PtySession> {
        if self.closing {
            return Err(session);
        }
        match self.slots.get_mut(&id) {
            Some(slot) => {
                slot.session = Some(session);
                Ok(())
            }
            None => Err(session),
        }
    }

    pub fn session(&self, id: u32) -> Option<&PtySession> {
        self.slots.get(&id)?.session.as_ref()
    }

    /// Coda di scrittura della sessione viva `id` e id del suo tab.
    pub fn writer(&self, id: u32) -> Option<(Sender<Vec<u8>>, String)> {
        let slot = self.slots.get(&id)?;
        slot.session.as_ref().map(|s| (s.writer.clone(), slot.tab_id.clone()))
    }

    /// Toglie dalla tabella la sessione viva `id` (non una prenotazione: e'
    /// di chi sta avviando la shell).
    pub fn take_session(&mut self, id: u32) -> Option<PtySession> {
        self.slots.get(&id)?.session.as_ref()?;
        self.release(id)?.session
    }

    /// Workspace del tab con una sessione viva.
    pub fn workspace_of_tab(&self, tab_id: &str) -> Option<&str> {
        let slot = self.slots.get(self.tabs.get(tab_id)?)?;
        slot.session.as_ref().map(|_| slot.workspace_id.as_str())
    }

    /// Da ora non si aprono altre sessioni; restituisce quelle vive, tolte
    /// dalla tabella, da terminare.
    pub fn begin_close(&mut self) -> Vec<PtySession> {
        self.closing = true;
        let live: Vec<u32> = self.slots.iter().filter(|(_, s)| s.session.is_some()).map(|(id, _)| *id).collect();
        live.into_iter().filter_map(|id| self.take_session(id)).collect()
    }
}

pub(super) type SharedRegistry = Arc<Mutex<Registry>>;

/// Slot prenotato per una sessione in avvio. Se non viene confermato (spawn
/// fallito, panic) il drop lo libera.
pub(super) struct Reservation {
    registry: SharedRegistry,
    id: u32,
    committed: bool,
}

impl Reservation {
    pub fn new(registry: &SharedRegistry, tab_id: &str, workspace_id: &str) -> Result<Self, ReserveError> {
        let id = registry.lock().unwrap().reserve(tab_id, workspace_id)?;
        Ok(Self { registry: registry.clone(), id, committed: false })
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// Rende viva la sessione nello slot prenotato.
    pub fn commit(mut self, session: PtySession) -> Result<u32, String> {
        let result = self.registry.lock().unwrap().commit(self.id, session);
        match result {
            Ok(()) => {
                self.committed = true;
                Ok(self.id)
            }
            Err(rejected) => {
                rejected.shutdown();
                Err(ReserveError::Closing.to_string())
            }
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if !self.committed {
            if let Ok(mut registry) = self.registry.lock() {
                registry.release(self.id);
            }
        }
    }
}
