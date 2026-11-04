//! Proposta di modifica delle impostazioni di un workspace: tipo della patch,
//! lettura degli argomenti del tool, validazione (le stesse funzioni dei comandi
//! dell'interfaccia, dall'API pubblica di `validate`) e applicazione
//! tutto-o-niente. Gli errori sono `ValidationError`: italiano per
//! l'interfaccia, inglese per il modello.

mod normalize;
mod sections;
#[cfg(test)]
mod tests;

use super::delivery::{Delivery, DeliveryMode, MergeMethod};
use super::e2e::E2eConfig;
use super::launches::clear_default_if_removed;
use super::model::{Launch, Workspace};
use super::repo_config::RepoConfig;
use super::store::{WorkspaceStore, LAUNCH_ID_PREFIX};
use super::validate::{
    check_default_launch, normalize_agent_worker, normalize_rules, normalize_workspace_agent, parse_worker_permission,
    repo_config_key, validate_launch,
};
use super::worker_settings::AgentWorkerSettings;
use crate::store::data_dir;
use crate::validate::ValidationError;
use normalize::{normalize_delivery, normalize_launches, normalize_repos, normalize_worker_agents, replace_launches};
use sections::de_double_option;
pub use sections::{AgentWorkerPatch, DeliveryPatch, LaunchPatch, RepoPatch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

/// Massimo di repository in una proposta di impostazioni.
pub const MAX_PATCH_REPOS: usize = 20;
/// Massimo di agenti in `worker_agents` di una proposta.
pub const MAX_PATCH_AGENTS: usize = 10;
/// Massimo di avvii nella lista proposta.
pub const MAX_PATCH_LAUNCHES: usize = 30;

/// Campi del workspace che una proposta non puo' toccare.
const FIXED_FIELDS: [&str; 8] = ["id", "name", "path", "kind", "distro", "shell", "archived", "created_at"];

/// Modifica proposta alle impostazioni del workspace. Solo i campi presenti
/// cambiano; le impostazioni globali e i dati fissi del workspace non esistono
/// nel tipo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct SettingsPatch {
    /// Testo intero; "" = nessuna regola.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub worker_rules: Option<String>,
    /// Agente dei worker e dei revisori; `null` = il predefinito globale.
    #[serde(deserialize_with = "de_double_option", skip_serializing_if = "Option::is_none")]
    // `Option<Option<T>>` distingue assente da `null`: in TS e' `campo?: T | null`.
    #[ts(as = "Option<String>", optional = nullable)]
    pub agent: Option<Option<String>>,
    /// plan | default | accept-edits | bypass.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub worker_permission: Option<String>,
    /// Modello e regole di permesso per agente (chiave = id dell'integrazione):
    /// di ogni agente cambiano solo i campi presenti.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub worker_agents: Option<BTreeMap<String, AgentWorkerPatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub delivery: Option<DeliveryPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub use_schedule: Option<bool>,
    /// `null` = eredita il predefinito globale.
    #[serde(deserialize_with = "de_double_option", skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<String>", optional = nullable)]
    pub default_launch: Option<Option<String>>,
    /// Lista intera, al posto di quella attuale.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub launches: Option<Vec<LaunchPatch>>,
    /// Chiave = cartella del repository; valore `null` = rimuove la configurazione.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repos: Option<BTreeMap<String, Option<RepoPatch>>>,
}

impl SettingsPatch {
    /// Legge gli argomenti del tool (oggetto SENZA `reason`). Errori in inglese.
    pub fn parse(v: &serde_json::Value) -> Result<SettingsPatch, String> {
        let obj = v.as_object().ok_or_else(|| "invalid settings: expected a JSON object".to_string())?;
        if let Some(field) = FIXED_FIELDS.iter().find(|f| obj.contains_key(**f)) {
            return Err(format!("`{field}` cannot be changed by a proposal (fixed: {})", FIXED_FIELDS.join(", ")));
        }
        SettingsPatch::deserialize(v).map_err(|e| format!("invalid settings: {e}"))
    }

    pub fn is_empty(&self) -> bool {
        *self == SettingsPatch::default()
    }

    /// Valida come i comandi dell'interfaccia e ritorna la patch con chiavi e
    /// valori normalizzati.
    pub fn normalized(&self, ws: &Workspace) -> Result<SettingsPatch, ValidationError> {
        let launches = self.launches.as_deref().map(|l| normalize_launches(l, &ws.launches)).transpose()?;
        Ok(SettingsPatch {
            worker_rules: self.worker_rules.as_deref().map(normalize_rules).transpose()?,
            agent: self.agent.clone().map(normalize_workspace_agent).transpose()?,
            worker_permission: self
                .worker_permission
                .as_deref()
                .map(|m| parse_worker_permission(m).map(|m| m.as_str().to_string()))
                .transpose()?,
            worker_agents: self.worker_agents.as_ref().map(|m| normalize_worker_agents(m, ws)).transpose()?,
            delivery: self.delivery.as_ref().map(|d| normalize_delivery(d, ws)).transpose()?,
            use_schedule: self.use_schedule,
            default_launch: self.normalized_default_launch(ws, launches.as_deref())?,
            launches,
            repos: self.repos.as_ref().map(|m| normalize_repos(m, ws)).transpose()?,
        })
    }

    /// `default_launch` verificato sulla lista di avvii DOPO la patch.
    fn normalized_default_launch(
        &self,
        ws: &Workspace,
        launches: Option<&[LaunchPatch]>,
    ) -> Result<Option<Option<String>>, ValidationError> {
        let Some(choice) = &self.default_launch else { return Ok(None) };
        let Some(id) = choice else { return Ok(Some(None)) };
        let id = id.trim();
        // Gli avvii nuovi non hanno ancora un id: non si possono referenziare.
        let after: Vec<Launch> = match launches {
            Some(list) => ws
                .launches
                .iter()
                .filter(|l| list.iter().any(|p| p.id.as_deref() == Some(l.id.as_str())))
                .cloned()
                .collect(),
            None => ws.launches.clone(),
        };
        // L'agente del workspace dopo la patch decide con chi partono i preset.
        let agent = self.agent.as_ref().map_or(ws.agent.as_deref(), |a| a.as_deref());
        let agent = agent.map(str::trim).filter(|a| !a.is_empty());
        check_default_launch(&after, id, &crate::agents::builtin_launch_list_for(agent))?;
        Ok(Some(Some(id.to_string())))
    }

    /// Applica la patch tutto-o-niente: ogni valore si valida su una copia.
    /// `new_launch_id` genera l'id degli avvii senza `id`.
    pub fn apply(&self, ws: &mut Workspace, new_launch_id: &mut dyn FnMut() -> String) -> Result<(), ValidationError> {
        let norm = self.normalized(ws)?;
        let mut next = ws.clone();
        if let Some(r) = norm.worker_rules {
            next.worker.rules = r;
        }
        if let Some(a) = norm.agent {
            next.agent = a;
        }
        if let Some(p) = norm.worker_permission {
            next.worker.permission = parse_worker_permission(&p)?;
        }
        for (id, patch) in norm.worker_agents.unwrap_or_default() {
            let base = next.worker.for_agent(&id).cloned().unwrap_or_default();
            next.worker.set_agent(&id, patch.merge(&base));
        }
        if let Some(d) = norm.delivery {
            next.delivery = d.merge(next.delivery);
        }
        if let Some(u) = norm.use_schedule {
            next.use_schedule = u;
        }
        if let Some(list) = norm.launches {
            replace_launches(&mut next, list, new_launch_id);
        }
        if let Some(d) = norm.default_launch {
            next.default_launch = d;
        }
        for (key, patch) in norm.repos.unwrap_or_default() {
            match patch {
                Some(p) => {
                    let base = next.repos.get(&key).cloned().unwrap_or_default();
                    next.repos.insert(key.clone(), p.merge(&base).map_err(|e| e.in_repo(&key))?);
                }
                None => {
                    next.repos.remove(&key);
                }
            }
        }
        *ws = next;
        Ok(())
    }

    /// Valori attuali dei soli campi toccati (per il confronto nell'interfaccia).
    pub fn current(&self, ws: &Workspace) -> SettingsPatch {
        SettingsPatch {
            worker_rules: self.worker_rules.as_ref().map(|_| ws.worker.rules.clone()),
            agent: self.agent.as_ref().map(|_| ws.agent.clone()),
            worker_permission: self.worker_permission.as_ref().map(|_| ws.worker.permission.as_str().to_string()),
            worker_agents: self.worker_agents.as_ref().map(|m| {
                m.iter()
                    .map(|(id, p)| (id.clone(), p.project(&ws.worker.for_agent(id).cloned().unwrap_or_default())))
                    .collect()
            }),
            delivery: self.delivery.as_ref().map(|d| d.project(ws.delivery)),
            use_schedule: self.use_schedule.map(|_| ws.use_schedule),
            default_launch: self.default_launch.as_ref().map(|_| ws.default_launch.clone()),
            launches: self.launches.as_ref().map(|_| ws.launches.iter().map(LaunchPatch::from).collect()),
            repos: self.repos.as_ref().map(|m| {
                m.iter()
                    .map(|(k, p)| {
                        let current = ws.repos.get(k).map(|cfg| match p {
                            Some(p) => p.project(cfg),
                            None => RepoPatch::full(cfg),
                        });
                        (k.clone(), current)
                    })
                    .collect()
            }),
        }
    }

    /// Campi toccati che eseguono comandi o allargano i permessi, per
    /// l'interfaccia. Ordine: `worker_permission`; per ogni agente di
    /// `worker_agents` (chiavi ordinate) `worker_agents[<id>].allow` e
    /// `worker_agents[<id>].deny` se presenti; `launches`, `default_launch`;
    /// poi, per ogni repository
    /// (chiavi ordinate) con patch non nulla, `repos[<chiave>].setup` e
    /// `repos[<chiave>].cleanup` se presenti, e `repos[<chiave>].e2e` se
    /// impostato (es. `repos[.].setup`, `repos[api/web].cleanup`,
    /// `repos[.].e2e`). Le rimozioni non sono sensibili.
    pub fn sensitive(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.worker_permission.is_some() {
            out.push("worker_permission".to_string());
        }
        for (id, patch) in self.worker_agents.iter().flatten() {
            if patch.allow.is_some() {
                out.push(format!("worker_agents[{id}].allow"));
            }
            if patch.deny.is_some() {
                out.push(format!("worker_agents[{id}].deny"));
            }
        }
        let direct = [("launches", self.launches.is_some()), ("default_launch", self.default_launch.is_some())];
        out.extend(direct.iter().filter(|(_, touched)| *touched).map(|(n, _)| n.to_string()));
        for (key, patch) in self.repos.iter().flatten() {
            let Some(p) = patch else { continue };
            if p.setup.is_some() {
                out.push(format!("repos[{key}].setup"));
            }
            if p.cleanup.is_some() {
                out.push(format!("repos[{key}].cleanup"));
            }
            if matches!(p.e2e, Some(Some(_))) {
                out.push(format!("repos[{key}].e2e"));
            }
        }
        out
    }

    /// Consegna o fasce orarie toccate: lo scheduler va riallineato.
    pub fn needs_reconcile(&self) -> bool {
        self.delivery.is_some() || self.use_schedule.is_some()
    }

    /// Chiavi dei repository toccati (normalizzate se la patch lo e').
    pub fn repo_keys(&self) -> Vec<String> {
        self.repos.iter().flatten().map(|(k, _)| k.clone()).collect()
    }
}

/// Applica una patch alle impostazioni di un workspace e salva su disco
/// (stesso percorso delle impostazioni dell'interfaccia). Ritorna il workspace.
/// Non riallinea lo scheduler: vedi `needs_reconcile`.
pub fn apply_settings_patch(app: &AppHandle, workspace_id: &str, patch: &SettingsPatch) -> Result<Workspace, String> {
    let dir = data_dir(app)?;
    let store = app.state::<WorkspaceStore>();
    store.modify(&dir, workspace_id, |ws| {
        patch.apply(ws, &mut || store.next_id(LAUNCH_ID_PREFIX))?;
        Ok(())
    })
}
