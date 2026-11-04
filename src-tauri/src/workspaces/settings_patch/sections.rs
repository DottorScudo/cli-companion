//! Le parti di una proposta di impostazioni: avvii, consegna e configurazione
//! di un repository. Ogni parte sa unirsi al valore attuale e riportare i soli
//! campi toccati.

use super::{Delivery, DeliveryMode, E2eConfig, Launch, MergeMethod, RepoConfig};
use crate::validate::ValidationError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Un campo `Option<Option<T>>` distingue "assente" (nessuna modifica) da
/// `null` (azzera il valore).
pub(super) fn de_double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Avvio nella lista proposta: senza `id` e' nuovo (id generato all'applicazione).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LaunchPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<String>,
    pub name: String,
    pub command: String,
}

impl From<&Launch> for LaunchPatch {
    fn from(l: &Launch) -> Self {
        LaunchPatch { id: Some(l.id.clone()), name: l.name.clone(), command: l.command.clone() }
    }
}

/// Campi di consegna da cambiare; gli assenti restano.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DeliveryPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mode: Option<DeliveryMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parallel: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub merge_method: Option<MergeMethod>,
}

impl DeliveryPatch {
    pub(super) fn is_empty(&self) -> bool {
        *self == DeliveryPatch::default()
    }

    /// `base` con i campi della patch sovrapposti, poi normalizzato.
    pub(super) fn merge(&self, base: Delivery) -> Delivery {
        Delivery {
            mode: self.mode.unwrap_or(base.mode),
            parallel: self.parallel.unwrap_or(base.parallel),
            merge_method: self.merge_method.unwrap_or(base.merge_method),
        }
        .normalized()
    }

    /// Gli stessi campi della patch, con i valori di `d`.
    pub(super) fn project(&self, d: Delivery) -> DeliveryPatch {
        DeliveryPatch {
            mode: self.mode.map(|_| d.mode),
            parallel: self.parallel.map(|_| d.parallel),
            merge_method: self.merge_method.map(|_| d.merge_method),
        }
    }
}

/// Modifica parziale della configurazione di un repository: solo i campi
/// presenti cambiano (`null` su `base_branch` / `container_root` li azzera).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct RepoPatch {
    #[serde(default, deserialize_with = "de_double_option", skip_serializing_if = "Option::is_none")]
    // `Option<Option<T>>` distingue assente da `null`: in TS e' `campo?: T | null`.
    #[ts(as = "Option<String>", optional = nullable)]
    pub base_branch: Option<Option<String>>,
    #[serde(default, deserialize_with = "de_double_option", skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<String>", optional = nullable)]
    pub container_root: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub copy_ignored: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cleanup: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remove_force: Option<bool>,
    /// Oggetto intero al posto di quello attuale; `null` lo rimuove.
    #[serde(default, deserialize_with = "de_double_option", skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<E2eConfig>", optional = nullable)]
    pub e2e: Option<Option<E2eConfig>>,
}

impl RepoPatch {
    pub fn is_empty(&self) -> bool {
        *self == RepoPatch::default()
    }

    /// `base` con i campi della patch sovrapposti, poi normalizzato.
    pub(super) fn merge(&self, base: &RepoConfig) -> Result<RepoConfig, ValidationError> {
        let mut c = base.clone();
        if let Some(v) = &self.base_branch {
            c.base_branch = v.clone();
        }
        if let Some(v) = &self.container_root {
            c.container_root = v.clone();
        }
        if let Some(v) = &self.copy_ignored {
            c.copy_ignored = v.clone();
        }
        if let Some(v) = &self.setup {
            c.setup = v.clone();
        }
        if let Some(v) = &self.cleanup {
            c.cleanup = v.clone();
        }
        if let Some(v) = self.remove_force {
            c.remove_force = v;
        }
        if let Some(v) = &self.e2e {
            c.e2e = v.clone();
        }
        c.normalized()
    }

    /// Gli stessi campi della patch, con i valori di `cfg`.
    pub(super) fn project(&self, cfg: &RepoConfig) -> RepoPatch {
        RepoPatch {
            base_branch: self.base_branch.as_ref().map(|_| cfg.base_branch.clone()),
            container_root: self.container_root.as_ref().map(|_| cfg.container_root.clone()),
            copy_ignored: self.copy_ignored.as_ref().map(|_| cfg.copy_ignored.clone()),
            setup: self.setup.as_ref().map(|_| cfg.setup.clone()),
            cleanup: self.cleanup.as_ref().map(|_| cfg.cleanup.clone()),
            remove_force: self.remove_force.map(|_| cfg.remove_force),
            e2e: self.e2e.as_ref().map(|_| cfg.e2e.clone()),
        }
    }

    /// Tutta la configurazione, ogni campo presente.
    pub(super) fn full(cfg: &RepoConfig) -> RepoPatch {
        RepoPatch {
            base_branch: Some(cfg.base_branch.clone()),
            container_root: Some(cfg.container_root.clone()),
            copy_ignored: Some(cfg.copy_ignored.clone()),
            setup: Some(cfg.setup.clone()),
            cleanup: Some(cfg.cleanup.clone()),
            remove_force: Some(cfg.remove_force),
            e2e: Some(cfg.e2e.clone()),
        }
    }
}
