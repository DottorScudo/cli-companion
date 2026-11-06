//! La skill del Bot di un task: come la carica l'agente del worker e cosa ne
//! legge il worker in `get_task`. L'esistenza di una skill utente la verifica
//! l'avvio (vedi `worktrees::start`), perche' serve il disco.

use crate::agents::{registry, worker_skill_invocation};
use crate::board::{texts, Task};
use serde_json::Value;

/// Il nome con cui l'agente del task carica la sua skill; `None` se il task non
/// ne ha una o se l'agente non sa caricarla.
pub(super) fn invocation(task: &Task) -> Option<String> {
    let skill = task.skill.as_ref()?;
    worker_skill_invocation(registry::find(&task.agent).ok()?, skill.launch_skill())
}

/// Aggiunge a una risposta di `get_task` il campo `skill` e, in testa alle
/// istruzioni, la riga che chiede di caricarla. Senza skill non cambia nulla.
pub(super) fn add_to_task_json(out: &mut Value, task: &Task) {
    let Some(invocation) = invocation(task) else {
        return;
    };
    if let Some(instructions) = out["instructions"].as_str() {
        out["instructions"] = format!("{}\n\n{instructions}", texts::skill_instruction(&invocation)).into();
    }
    out["skill"] = invocation.into();
}
