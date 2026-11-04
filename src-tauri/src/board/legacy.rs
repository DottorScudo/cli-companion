//! Migrazioni del file della board. La versione 0 e' ogni file senza busta: il
//! formato di oggi (task con `status` e campi piatti) e quello della v0.11,
//! che aveva stati e campi poi eliminati. Il passo 0 -> 1 porta la v0.11 al
//! modello attuale lavorando sul JSON, cosi' i tipi non portano stati che non
//! esistono piu'; l'interruzione dei task `running` NON e' una migrazione: la fa
//! `Board::recover_after_restart` a ogni avvio (vedi `recovery`).

use serde_json::Value;
use std::collections::HashSet;

const UNINTEGRATED_NOTE: &str = "completed but not integrated (board v0.11): retry to let the worker deliver it";

fn status_of(task: &Value) -> &str {
    task.get("status").and_then(Value::as_str).unwrap_or("queued")
}

/// Passo 0 -> 1: stati e campi della v0.11 verso il modello attuale (vedi
/// docs/BOARD.MD, "Migrazione dello schema"). Un file gia' attuale passa
/// invariato, salvo i tab dei worker dei task non in corso, che non esistono piu'.
pub(super) fn v0_to_v1(mut v: Value) -> Result<Value, String> {
    let Some(tasks) = v.get_mut("tasks").and_then(Value::as_array_mut) else {
        return Ok(v);
    };
    tasks.retain(|t| status_of(t) != "cancelled");
    let alive: HashSet<String> = tasks.iter().filter_map(|t| t.get("id")?.as_str().map(str::to_string)).collect();
    for t in tasks.iter_mut() {
        let status = status_of(t).to_string();
        let Some(o) = t.as_object_mut() else {
            continue;
        };
        let (new, note) = match status.as_str() {
            "queued" => ("queued", None),
            "done" => ("done", None),
            "running" => ("running", None),
            "review" => ("failed", Some(UNINTEGRATED_NOTE)),
            _ => ("failed", None), // blocked, failed
        };
        o.insert("status".into(), new.into());
        if let Some(n) = note {
            o.insert("note".into(), n.into());
        }
        if new != "running" {
            // I tab dei worker non si ripristinano.
            o.insert("session".into(), Value::Null);
            o.insert("worker_status".into(), Value::Null);
        }
        if let Some(deps) = o.get_mut("depends_on").and_then(Value::as_array_mut) {
            deps.retain(|d| d.as_str().is_some_and(|d| alive.contains(d)));
        }
        if let Some(r) = o.get_mut("report").and_then(Value::as_object_mut) {
            if r.get("status").and_then(Value::as_str) == Some("blocked") {
                r.insert("status".into(), "failed".into());
            }
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests;
