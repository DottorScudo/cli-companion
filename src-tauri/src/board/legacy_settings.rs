//! Migrazione del file della board per le impostazioni dei worker per agente.
//! Le proposte di impostazioni pendenti contengono una `SettingsPatch` nel
//! formato storico, con `worker_model`, `worker_allow` e `worker_deny` validi
//! per ogni agente; ora quei campi stanno in `worker_agents[<agente>]` (vedi
//! `workspaces::migrations`, che fa lo stesso per i workspace).

use crate::agents::registry;
use crate::workspaces::take_legacy_worker_fields;
use serde_json::{json, Value};

/// Chiave della patch con le impostazioni dei worker per agente.
const WORKER_AGENTS_KEY: &str = "worker_agents";

/// Passo 1 -> 2: i campi storici di ogni proposta passano alla voce
/// dell'agente predefinito di adesso, quello che i worker usavano.
pub(super) fn v1_to_v2(board: Value) -> Result<Value, String> {
    Ok(move_patch_fields(board, registry::default_agent().id()))
}

fn move_patch_fields(mut board: Value, agent_id: &str) -> Value {
    let patches = board
        .get_mut("settings_proposals")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(|proposal| proposal.get_mut("patch")?.as_object_mut());
    for patch in patches {
        let legacy = take_legacy_worker_fields(patch);
        if legacy.is_empty() {
            continue;
        }
        let agents = patch.entry(WORKER_AGENTS_KEY).or_insert_with(|| json!({}));
        if let Some(agents) = agents.as_object_mut() {
            agents.entry(agent_id).or_insert(Value::Object(legacy));
        }
    }
    board
}

#[cfg(test)]
mod tests;
