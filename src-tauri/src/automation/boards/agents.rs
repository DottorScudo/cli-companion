//! Task in coda con l'agente non pronto: il gate degli agenti per workspace.
//! Lo scheduler non avvia un task il cui agente non e' pronto nel target del
//! workspace (non fallisce: aspetta); qui si calcola e si ricorda quel gate, e
//! si rivaluta quando la prontezza puo' essere cambiata (giro del timer, fine
//! di un controllo). La prontezza arriva da una funzione non bloccante, quindi
//! tutto questo gira sotto il lock del registro.

use super::*;
use crate::board::AgentGate;
use std::collections::HashSet;

/// Prontezza nota di un agente nel target del workspace, senza bloccare
/// (`Host::agent_readiness`); `None` = non nota ancora.
pub(in crate::automation) type ReadinessLookup<'a> = dyn Fn(&Workspace, &str) -> Option<Readiness> + 'a;

impl Registry {
    /// Il gate dei task in coda di `ws`, calcolato adesso. Un workspace
    /// archiviato e' congelato e non guarda gli agenti.
    fn compute_gate(&self, ws: &Workspace, readiness: &ReadinessLookup) -> AgentGate {
        match self.boards.get(&ws.id) {
            Some(b) if !ws.archived => AgentGate::build(b.queued_agents(), &mut |agent| readiness(ws, agent)),
            _ => AgentGate::default(),
        }
    }

    /// Ricalcola e ricorda il gate di `ws` (solo se non vuoto) e lo ritorna.
    pub(super) fn refresh_gate(&mut self, ws: &Workspace, readiness: &ReadinessLookup) -> AgentGate {
        let gate = self.compute_gate(ws, readiness);
        if gate.is_empty() {
            self.gates.remove(&ws.id);
        } else {
            self.gates.insert(ws.id.clone(), gate.clone());
        }
        gate
    }

    /// Il gate ricordato di `ws` (vuoto se nessun agente e' bloccato).
    pub(super) fn gate_of(&self, ws: &str) -> AgentGate {
        self.gates.get(ws).cloned().unwrap_or_default()
    }

    /// I workspace con task fermi per un agente rivalutano la prontezza: la
    /// board riparte solo se il gate e' cambiato (niente pubblicazioni a vuoto).
    pub(in crate::automation) fn recheck_agents(&mut self, workspaces: &[Workspace], readiness: &ReadinessLookup) {
        let changed: Vec<&Workspace> = workspaces
            .iter()
            .filter(|w| self.gates.get(&w.id).is_some_and(|stored| *stored != self.compute_gate(w, readiness)))
            .collect();
        for ws in changed {
            self.mark_dirty(&ws.id);
        }
    }

    /// Dimentica i gate dei workspace che non esistono piu'.
    pub(in crate::automation) fn retain_gates(&mut self, known: &HashSet<&str>) {
        self.gates.retain(|id, _| known.contains(id.as_str()));
    }
}

impl AutomationState {
    /// La prontezza di qualche agente e' cambiata: le board con task fermi per
    /// un agente si rivalutano subito, senza aspettare il giro del timer.
    pub fn recheck_agents<H: Host>(&self, host: &H) {
        let workspaces = host.workspaces();
        self.run(host, |r| r.recheck_agents(&workspaces, &|ws, agent| host.agent_readiness(ws, agent)));
    }
}
