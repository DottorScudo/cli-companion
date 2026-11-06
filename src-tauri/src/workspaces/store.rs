//! Lo stato in memoria dei workspace e la sua persistenza in `workspaces.json`
//! tramite `store::JsonStore` (busta versionata, migrazioni, backup, avvisi).
//! Ogni modifica passa da `update` / `modify`: si applica a una copia, si salva
//! e solo poi si pubblica.

use super::migrations::{migrate_v0, migrate_v1, migrate_v2, NOT_A_LIST};
use super::model::Workspace;
use crate::store::{data_dir, Decoded, JsonFile, JsonStore};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// Errore di un id che non corrisponde a nessun workspace.
pub(super) const WORKSPACE_NOT_FOUND: &str = "workspace non trovato";

/// Prefissi degli id generati.
pub(super) const WORKSPACE_ID_PREFIX: &str = "ws";
pub(super) const LAUNCH_ID_PREFIX: &str = "ln";

/// Descrizione del file: i test ne fanno istanze locali (il blocco delle scritture e' per istanza).
const fn workspaces_file() -> JsonFile<Vec<Workspace>> {
    JsonFile::new("workspaces.json", &[migrate_v0, migrate_v1, migrate_v2]).decoded_with(decode_workspaces)
}

static WORKSPACES_FILE: JsonFile<Vec<Workspace>> = workspaces_file();

/// Legge i workspace uno per uno: quello illeggibile viene escluso (e
/// riportato) senza far perdere gli altri.
fn decode_workspaces(data: Value) -> Result<Decoded<Vec<Workspace>>, String> {
    let Value::Array(entries) = data else { return Err(NOT_A_LIST.to_string()) };
    let mut decoded = Decoded { value: Vec::with_capacity(entries.len()), dropped: Vec::new() };
    for (index, entry) in entries.into_iter().enumerate() {
        let id = entry.get("id").and_then(Value::as_str).map(str::to_string);
        match serde_json::from_value::<Workspace>(entry) {
            Ok(ws) => decoded.value.push(ws),
            Err(e) => {
                let which = id.map_or_else(|| format!("voce {}", index + 1), |id| format!("workspace \"{id}\""));
                decoded.dropped.push(format!("{which}: {e}"));
            }
        }
    }
    Ok(decoded)
}

pub struct WorkspaceStore {
    list: JsonStore<Vec<Workspace>>,
    /// Contatore degli id generati (workspace e avvii): unico nello stesso millisecondo.
    sequence: AtomicU64,
}

impl Default for WorkspaceStore {
    fn default() -> Self {
        Self::with_file(&WORKSPACES_FILE)
    }
}

impl WorkspaceStore {
    fn with_file(file: &'static JsonFile<Vec<Workspace>>) -> Self {
        Self { list: JsonStore::new(file), sequence: AtomicU64::new(0) }
    }

    pub fn all(&self) -> Vec<Workspace> {
        self.list.get()
    }

    pub fn find(&self, id: &str) -> Option<Workspace> {
        self.list.read(|list| list.iter().find(|w| w.id == id).cloned())
    }

    /// Carica da `dir` (file assente = nessun workspace).
    pub fn load(&self, dir: &Path) {
        self.list.load(dir);
    }

    /// Nuovo id `<prefisso>-<millisecondi>-<n>`: il contatore garantisce l'unicita'
    /// nello stesso millisecondo.
    pub fn next_id(&self, prefix: &str) -> String {
        let n = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{}-{n}", now_millis())
    }

    /// Applica `f` all'elenco e lo salva in `dir`. Se `f` o il salvataggio
    /// falliscono l'elenco in memoria e il file restano come prima.
    pub fn update<T>(&self, dir: &Path, f: impl FnOnce(&mut Vec<Workspace>) -> Result<T, String>) -> Result<T, String> {
        let mut out = None;
        self.list.update(dir, |list| {
            out = Some(f(list)?);
            Ok(())
        })?;
        Ok(out.expect("la modifica ha prodotto un valore"))
    }

    /// Modifica il workspace `id` con `f`, salva e ritorna la sua nuova versione.
    pub fn modify(
        &self,
        dir: &Path,
        id: &str,
        f: impl FnOnce(&mut Workspace) -> Result<(), String>,
    ) -> Result<Workspace, String> {
        self.update(dir, |list| {
            let ws = find_mut(list, id)?;
            f(ws)?;
            Ok(ws.clone())
        })
    }
}

/// Istante corrente in millisecondi dall'epoca Unix.
pub(super) fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Il workspace `id` dell'elenco, se esiste.
pub(super) fn find_mut<'a>(list: &'a mut [Workspace], id: &str) -> Result<&'a mut Workspace, String> {
    list.iter_mut().find(|w| w.id == id).ok_or_else(|| WORKSPACE_NOT_FOUND.to_string())
}

/// Carica i workspace all'avvio dell'app.
pub fn load_from_disk(app: &tauri::App) {
    match data_dir(app.handle()) {
        Ok(dir) => app.state::<WorkspaceStore>().load(&dir),
        Err(e) => log::error!("cartella dati non disponibile, workspace non caricati: {e}"),
    }
}

/// "Modifica un workspace e salva" per i comandi: `f` cambia il workspace `id`,
/// il risultato e' il workspace salvato.
pub(super) fn edit(
    app: &AppHandle,
    id: &str,
    f: impl FnOnce(&mut Workspace) -> Result<(), String>,
) -> Result<Workspace, String> {
    let dir = data_dir(app)?;
    app.state::<WorkspaceStore>().modify(&dir, id, f)
}

/// Come `edit`, per le modifiche che non possono fallire.
pub(super) fn change(app: &AppHandle, id: &str, f: impl FnOnce(&mut Workspace)) -> Result<Workspace, String> {
    edit(app, id, |ws| {
        f(ws);
        Ok(())
    })
}

#[cfg(test)]
mod tests;
