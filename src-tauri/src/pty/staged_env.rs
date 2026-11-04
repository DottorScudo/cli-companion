//! Variabili d'ambiente preparate dal backend per il PTY di un tab che il
//! frontend sta per aprire (oggi: i worker, con le variabili che la loro
//! integrazione chiede in `AutomationLaunch::env`).
//!
//! Vive dentro `PtyState` (stato gestito dall'app, come il registro delle
//! sessioni) e non in una `static`: ha il ciclo di vita dell'app, non e'
//! condiviso fra le istanze dei test e `pty_spawn` la raggiunge dallo stesso
//! `AppHandle` da cui prende il registro. Il frontend non puo' impostare
//! variabili: `pty_spawn` non ha parametri d'ambiente, legge soltanto qui,
//! con la chiave `tab_id` che il backend ha scelto.
//!
//! Una voce si consuma una volta sola (`take`). Quelle di tab che non
//! vengono mai aperti non si accumulano: si scartano alla chiusura del
//! worker (`discard`) e comunque il numero e' limitato (le piu' vecchie
//! cedono il posto).

use std::collections::VecDeque;
use std::sync::Mutex;

/// Voci conservate al massimo: oltre, la piu' vecchia viene scartata.
pub(super) const MAX_STAGED: usize = 64;

type Vars = Vec<(String, String)>;

#[derive(Default)]
pub(super) struct StagedEnv {
    entries: Mutex<VecDeque<(String, Vars)>>,
}

impl StagedEnv {
    /// Prepara `vars` per il tab `tab_id` (sostituisce una voce precedente).
    pub fn stage(&self, tab_id: &str, vars: Vars) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(id, _)| id != tab_id);
        entries.push_back((tab_id.to_string(), vars));
        while entries.len() > MAX_STAGED {
            entries.pop_front();
        }
    }

    /// Toglie e restituisce le variabili del tab (vuote se non ce ne sono).
    pub fn take(&self, tab_id: &str) -> Vars {
        let mut entries = self.entries.lock().unwrap();
        let Some(pos) = entries.iter().position(|(id, _)| id == tab_id) else { return Vec::new() };
        entries.remove(pos).map(|(_, vars)| vars).unwrap_or_default()
    }

    /// Scarta la voce del tab, se c'e'.
    pub fn discard(&self, tab_id: &str) {
        self.entries.lock().unwrap().retain(|(id, _)| id != tab_id);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}
