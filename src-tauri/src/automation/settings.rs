//! Tool MCP delle impostazioni del workspace (`get_workspace_settings`,
//! `propose_settings`) e decisione dell'utente sulle proposte. L'agente legge la
//! configurazione e propone modifiche; nulla cambia finche' l'utente non
//! approva (`settings_proposal_decide`). Il tipo della patch e la sua
//! validazione sono in `crate::workspaces`, lo stoccaggio in `crate::board`.

use super::boards::{session_workspace, workspace_of};
use super::rpc::misrouted;
use super::*;
use crate::agents::{builtin_launch_list, registry, worker_rule_format, AgentIntegration, PermissionMode, RuleFormat};
use crate::board::SettingsProposal;
use crate::workspaces::{
    DeliveryMode, MergeMethod, SettingsPatch, MAX_COMMAND_LEN, MAX_LAUNCH_NAME_LEN, MAX_PATCH_AGENTS,
    MAX_PATCH_LAUNCHES, MAX_PATCH_REPOS, MAX_REPO_COMMANDS, MAX_REPO_COMMAND_LEN, MAX_RULES_LEN,
};
use serde_json::json;
use std::collections::BTreeMap;
use ts_rs::TS;

/// Lunghezza massima della motivazione di una proposta.
pub(crate) const REASON_MAX: usize = 2000;

/// Avvio predefinito "shell semplice".
const SHELL_LAUNCH: &str = "shell";

pub(super) fn rpc<H: Host>(
    st: &AutomationState,
    host: &H,
    session: &str,
    method: Method,
    params: &Value,
) -> Result<Value, String> {
    let ws = session_workspace(host, session)?;
    match method {
        Method::GetWorkspaceSettings => Ok(get_settings(st, host, &ws)),
        Method::ProposeSettings => propose_settings(st, host, session, &ws, params),
        other => Err(misrouted(other)),
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

impl Registry {
    /// Nuova proposta di impostazioni: ritorna l'id.
    fn propose_settings(
        &mut self,
        ws: &str,
        source_tab: &str,
        reason: String,
        patch: SettingsPatch,
        now: i64,
    ) -> Result<String, String> {
        self.next_proposal += 1;
        let id = format!("s-{now}-{}", self.next_proposal);
        self.edit(ws, |b| b.add_settings_proposal(id.clone(), source_tab, reason, patch, now))?;
        Ok(id)
    }

    fn settings_proposal(&self, ws: &str, id: &str) -> Result<SettingsProposal, String> {
        self.boards.get(ws).ok_or_else(|| format!("settings proposal not found: {id}"))?.settings_proposal(id).cloned()
    }

    fn remove_settings_proposal(&mut self, ws: &str, id: &str, now: i64) -> Result<(), String> {
        self.edit(ws, |b| b.remove_settings_proposal(id, now).map(drop))
    }

    /// `(id, motivazione)` delle proposte di impostazioni pendenti.
    fn pending_settings(&self, ws: &str) -> Vec<(String, String)> {
        let proposals = self.boards.get(ws).map(|b| b.settings_proposals.as_slice()).unwrap_or_default();
        proposals.iter().map(|p| (p.id.clone(), p.reason.clone())).collect()
    }
}

// ---------------------------------------------------------------------------
// get_workspace_settings
// ---------------------------------------------------------------------------

fn get_settings<H: Host>(st: &AutomationState, host: &H, ws: &Workspace) -> Value {
    // Git e disco fuori dal lock; un errore non impedisce di leggere il resto.
    let (detected, repos_error) = match host.list_repos(ws) {
        Ok(detected) => (detected, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let pending: Vec<Value> = st
        .run(host, |r| r.pending_settings(&ws.id))
        .into_iter()
        .map(|(id, reason)| json!({ "id": id, "reason": reason }))
        .collect();
    let mut out = json!({
        "settings": settings_json(ws),
        "agents": agents_json(host, ws),
        "repos": repos_json(ws, &detected),
        "allowed": allowed_json(ws),
        "limits": limits_json(),
        "pending_settings_proposals": pending,
    });
    if let Some(e) = repos_error {
        out["repos_error"] = Value::String(e);
    }
    out
}

/// Le impostazioni che `propose_settings` puo' cambiare, con i loro valori.
fn settings_json(ws: &Workspace) -> Value {
    json!({
        "worker_rules": ws.worker.rules,
        "agent": ws.agent,
        "worker_permission": ws.worker.permission.as_str(),
        "worker_agents": ws.worker.agents,
        "delivery": ws.delivery,
        "use_schedule": ws.use_schedule,
        "default_launch": ws.default_launch,
        "launches": ws.launches,
    })
}

/// Gli agenti abilitati, in ordine di registro: ognuno con i modelli e la forma
/// delle regole di permesso che `worker_agents` accetta per lui, piu' la prontezza
/// nel target del workspace (puo' lanciare processi: fuori dai lock).
fn agents_json<H: Host>(host: &H, ws: &Workspace) -> Vec<Value> {
    let default_id = registry::default_agent().id();
    registry::enabled().into_iter().map(|agent| agent_json(host, ws, agent, default_id)).collect()
}

fn agent_json<H: Host>(host: &H, ws: &Workspace, agent: &dyn AgentIntegration, default_id: &str) -> Value {
    let capabilities = agent.capabilities();
    json!({
        "id": agent.id(),
        "name": agent.display_name(),
        "is_default": agent.id() == default_id,
        "models": capabilities.models.map_or(&[][..], |catalog| catalog.suggested()),
        "default_model": capabilities.models.and_then(|catalog| catalog.default_model()),
        "rule_format": worker_rule_format(agent.id()).map(rule_format_json),
        "readiness": host.probe_agent(ws, agent.id()),
    })
}

fn rule_format_json(format: RuleFormat) -> Value {
    json!({
        "max_rules": format.max_rules,
        "max_rule_chars": format.max_rule_chars,
        "example": format.example,
    })
}

/// Repository rilevati e repository con una configurazione, per chiave.
fn repos_json(ws: &Workspace, detected: &[String]) -> Vec<Value> {
    let mut repos: BTreeMap<&str, bool> = ws.repos.keys().map(|k| (k.as_str(), false)).collect();
    repos.extend(detected.iter().map(|k| (k.as_str(), true)));
    repos
        .into_iter()
        .map(|(repo, detected)| json!({ "repo": repo, "detected": detected, "config": ws.repos.get(repo) }))
        .collect()
}

fn allowed_json(ws: &Workspace) -> Value {
    let launches = std::iter::once(SHELL_LAUNCH.to_string())
        .chain(ws.launches.iter().map(|l| l.id.clone()))
        .chain(builtin_launch_list().into_iter().map(|b| b.id))
        .collect::<Vec<_>>();
    json!({
        "worker_permission": PermissionMode::ALL.map(PermissionMode::as_str),
        "delivery_mode": DeliveryMode::ALL.map(DeliveryMode::as_str),
        "merge_method": MergeMethod::ALL.map(MergeMethod::as_str),
        "default_launch": launches,
    })
}

/// Limiti delle proposte. Quelli delle regole di permesso dipendono dall'agente
/// e stanno in `agents[].rule_format`.
pub(super) fn limits_json() -> Value {
    json!({
        "reason_chars": REASON_MAX,
        "worker_rules_chars": MAX_RULES_LEN,
        "launches": MAX_PATCH_LAUNCHES,
        "launch_name_chars": MAX_LAUNCH_NAME_LEN,
        "launch_command_chars": MAX_COMMAND_LEN,
        "repos_per_proposal": MAX_PATCH_REPOS,
        "repo_commands": MAX_REPO_COMMANDS,
        "repo_command_chars": MAX_REPO_COMMAND_LEN,
        "agents_per_proposal": MAX_PATCH_AGENTS,
    })
}

// ---------------------------------------------------------------------------
// propose_settings
// ---------------------------------------------------------------------------

fn propose_settings<H: Host>(
    st: &AutomationState,
    host: &H,
    session: &str,
    ws: &Workspace,
    params: &Value,
) -> Result<Value, String> {
    let Some(fields) = params.as_object() else {
        return Err("params must be an object".to_string());
    };
    let reason = req_text("reason", req_str(params, "reason")?, REASON_MAX)?;
    let mut fields = fields.clone();
    fields.remove("reason");
    let patch = SettingsPatch::parse(&Value::Object(fields))?;
    if patch.is_empty() {
        return Err("nothing to change: pass at least one setting besides reason".to_string());
    }
    let patch = patch.normalized(ws).map_err(|e| e.message_en())?;
    ensure_known_repos(host, ws, &patch.repo_keys())?;
    let now = now_millis();
    let id = st.run(host, |r| r.propose_settings(&ws.id, session, reason, patch, now))?;
    Ok(json!({
        "proposal": id,
        "status": "pending",
        "message": "The user approves or discards it in the app. There is no feedback after the decision: do not wait."
    }))
}

/// Ogni repository della proposta deve essere rilevato nel workspace o avere
/// gia' una configurazione (la rimozione di una configurazione orfana).
fn ensure_known_repos<H: Host>(host: &H, ws: &Workspace, keys: &[String]) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    let detected = host.list_repos(ws).map_err(|e| format!("cannot list the repositories of the workspace: {e}"))?;
    let unknown: Vec<&str> =
        keys.iter().filter(|k| !detected.contains(k) && !ws.repos.contains_key(*k)).map(String::as_str).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let known = if detected.is_empty() { "none".to_string() } else { detected.join(", ") };
    Err(format!("unknown repository: {} (detected repositories: {known})", unknown.join(", ")))
}

// ---------------------------------------------------------------------------
// Decisione dell'utente
// ---------------------------------------------------------------------------

/// Esito di una decisione: la board aggiornata e, se approvata, il workspace
/// con le nuove impostazioni.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SettingsDecision {
    pub board: BoardView,
    pub workspace: Option<Workspace>,
}

/// Approva o scarta una proposta di impostazioni. Approvando, la patch si
/// rivalida sulla configurazione attuale: se fallisce la proposta resta li'.
pub(super) fn settings_proposal_decide<H: Host>(
    st: &AutomationState,
    host: &H,
    ws_id: &str,
    proposal_id: &str,
    accept: bool,
) -> Result<SettingsDecision, String> {
    // Per tutta la decisione: un doppio clic non applica due volte la stessa
    // proposta e uno scarto non gareggia con un'approvazione.
    let _serial = lock(&st.settings_decisions);
    workspace_of(host, ws_id)?;
    let proposal = st.run(host, |r| r.settings_proposal(ws_id, proposal_id))?;
    let workspace = accept.then(|| host.apply_settings(ws_id, &proposal.patch)).transpose()?;
    st.run(host, |r| r.remove_settings_proposal(ws_id, proposal_id, now_millis()))?;
    if workspace.is_some() && proposal.patch.needs_reconcile() {
        st.reconcile(host);
    }
    Ok(SettingsDecision { board: st.view(host, ws_id)?, workspace })
}
