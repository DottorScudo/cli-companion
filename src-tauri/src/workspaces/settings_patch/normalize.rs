//! Validazione delle sezioni di una proposta (consegna, avvii, repository) con
//! le stesse regole dei comandi dell'interfaccia.

use super::sections::{DeliveryPatch, LaunchPatch, RepoPatch};
use super::{
    clear_default_if_removed, repo_config_key, validate_launch, Launch, Workspace, MAX_PATCH_LAUNCHES, MAX_PATCH_REPOS,
};
use crate::validate::{ErrorCode, ValidationError};
use std::collections::{BTreeMap, HashSet};

pub(super) fn normalize_delivery(d: &DeliveryPatch, ws: &Workspace) -> Result<DeliveryPatch, ValidationError> {
    if d.is_empty() {
        return Err(ValidationError::new(ErrorCode::DeliveryEmpty));
    }
    Ok(d.project(d.merge(ws.delivery)))
}

/// Lista di avvii proposta: ogni voce come l'interfaccia; un `id` deve
/// esistere gia' e comparire una volta sola.
pub(super) fn normalize_launches(
    list: &[LaunchPatch],
    current: &[Launch],
) -> Result<Vec<LaunchPatch>, ValidationError> {
    if list.len() > MAX_PATCH_LAUNCHES {
        return Err(ValidationError::new(ErrorCode::LaunchesTooMany).max(MAX_PATCH_LAUNCHES));
    }
    let mut seen = HashSet::new();
    list.iter()
        .enumerate()
        .map(|(index, p)| normalize_launch(p, current, &mut seen).map_err(|e| e.in_launch(index)))
        .collect()
}

fn normalize_launch<'a>(
    p: &'a LaunchPatch,
    current: &[Launch],
    seen: &mut HashSet<&'a str>,
) -> Result<LaunchPatch, ValidationError> {
    let (name, command) = validate_launch(&p.name, &p.command)?;
    if let Some(id) = p.id.as_deref() {
        if !current.iter().any(|l| l.id == id) {
            return Err(ValidationError::new(ErrorCode::LaunchIdUnknown).value(id));
        }
        if !seen.insert(id) {
            return Err(ValidationError::new(ErrorCode::LaunchIdDuplicate).value(id));
        }
    }
    Ok(LaunchPatch { id: p.id.clone(), name, command })
}

/// Sostituisce gli avvii nell'ordine della patch; quelli non elencati sono
/// rimossi (come `remove_launch`: se era il predefinito si torna a ereditare).
pub(super) fn replace_launches(ws: &mut Workspace, list: Vec<LaunchPatch>, new_launch_id: &mut dyn FnMut() -> String) {
    let old = std::mem::take(&mut ws.launches);
    ws.launches = list
        .into_iter()
        .map(|p| Launch { id: p.id.unwrap_or_else(&mut *new_launch_id), name: p.name, command: p.command })
        .collect();
    let removed: Vec<String> =
        old.into_iter().filter(|o| !ws.launches.iter().any(|l| l.id == o.id)).map(|o| o.id).collect();
    for id in &removed {
        clear_default_if_removed(ws, id);
    }
}

/// Repository proposti: chiavi normalizzate, senza doppioni, voci non vuote;
/// ogni errore porta la chiave. `None` (rimozione) passa.
pub(super) fn normalize_repos(
    map: &BTreeMap<String, Option<RepoPatch>>,
    ws: &Workspace,
) -> Result<BTreeMap<String, Option<RepoPatch>>, ValidationError> {
    if map.len() > MAX_PATCH_REPOS {
        return Err(ValidationError::new(ErrorCode::ReposTooMany).max(MAX_PATCH_REPOS));
    }
    let mut out = BTreeMap::new();
    for (raw, patch) in map {
        let key = repo_config_key(raw).map_err(|e| e.in_repo(raw))?;
        if out.contains_key(&key) {
            return Err(ValidationError::new(ErrorCode::RepoKeyDuplicate).in_repo(&key));
        }
        let entry = match patch {
            None => None,
            Some(p) if p.is_empty() => return Err(ValidationError::new(ErrorCode::RepoPatchEmpty).in_repo(&key)),
            Some(p) => {
                let base = ws.repos.get(&key).cloned().unwrap_or_default();
                Some(p.project(&p.merge(&base).map_err(|e| e.in_repo(&key))?))
            }
        };
        out.insert(key, entry);
    }
    Ok(out)
}
