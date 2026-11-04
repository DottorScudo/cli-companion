//! Gli agenti non pronti di un workspace e il motivo che la board mostra sui
//! task in coda che li aspettano. La prontezza la decide l'automazione (serve
//! sondare il target): qui c'e' solo il risultato, puro, che lo scheduler e la
//! vista leggono. Un task con l'agente non pronto non fallisce: resta in coda
//! e parte quando l'agente lo diventa.

use super::{Board, Task, TaskStatus};
use crate::agents::Readiness;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;

/// Agenti non pronti del workspace: `None` = la prontezza non e' ancora nota
/// (un controllo e' in corso), `Some(motivo)` = nota e diversa da `Ready`. Gli
/// agenti pronti non ci sono.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentGate(BTreeMap<String, Option<Readiness>>);

impl AgentGate {
    /// Il gate degli `agents` dati; `readiness` risponde per ogni agente (una
    /// volta sola, anche se ripetuto) con la prontezza nota o `None`.
    pub fn build<'a>(
        agents: impl IntoIterator<Item = &'a str>,
        readiness: &mut dyn FnMut(&str) -> Option<Readiness>,
    ) -> Self {
        let unique: BTreeSet<&str> = agents.into_iter().collect();
        let blocked = unique
            .into_iter()
            .filter_map(|agent| match readiness(agent) {
                Some(Readiness::Ready) => None,
                known => Some((agent.to_string(), known)),
            })
            .collect();
        Self(blocked)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// I task di `agent` non possono partire adesso (non pronto o ancora in
    /// controllo).
    pub fn is_blocked(&self, agent: &str) -> bool {
        self.0.contains_key(agent)
    }

    /// Perche' `agent` non e' pronto, se lo si sa gia'.
    pub fn reason(&self, agent: &str) -> Option<Readiness> {
        self.0.get(agent).copied().flatten()
    }

    /// Il motivo da mostrare su `task`: solo se e' in coda e la prontezza del
    /// suo agente e' nota e negativa.
    pub fn unavailable(&self, task: &Task) -> Option<AgentUnavailableView> {
        if task.status() != TaskStatus::Queued {
            return None;
        }
        self.reason(&task.agent).map(|reason| AgentUnavailableView { agent: task.agent.clone(), reason })
    }
}

/// Perche' un task in coda non parte: il suo agente non e' pronto nel target
/// del workspace. Parte da solo appena lo diventa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AgentUnavailableView {
    pub agent: String,
    pub reason: Readiness,
}

impl Board {
    /// Gli agenti dei task in coda: gli unici la cui prontezza conta adesso.
    pub fn queued_agents(&self) -> impl Iterator<Item = &str> {
        self.tasks.iter().filter(|t| t.status() == TaskStatus::Queued).map(|t| t.agent.as_str())
    }
}

#[cfg(test)]
mod tests;
