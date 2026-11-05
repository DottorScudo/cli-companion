//! Le azioni dell'interfaccia (prova, consenso, abbinamento): solo quelle
//! dichiarate dal descrittore. Per `pair` il runtime emette il codice monouso e lo
//! mette nel contesto (`ConnectorContext::pairing_code`) oltre a restituirlo
//! all'interfaccia. Un errore del connettore arriva all'utente solo come messaggio
//! generico della sua classe: il dettaglio resta nel log.

use super::checks::{check_action, check_instance_id, find_connector, INSTANCE_NOT_FOUND};
use super::instances::restart_if_needs_auth;
use super::view::ActionResult;
use crate::connectors::descriptor::ActionId;
use crate::connectors::limits::ACTION_TIMEOUT;
use crate::connectors::runtime::manager::ConnectorsRuntime;
use crate::connectors::runtime::time::with_timeout;
use tauri::{AppHandle, Manager};

const PAIR_NEEDS_RUNNING: &str = "Attiva il connettore (e l'HUB) prima di abbinare un account.";

/// Esegue un'azione dichiarata. `pair` richiede l'istanza in esecuzione (altrimenti
/// nessuno riceverebbe il codice).
pub async fn execute(rt: &ConnectorsRuntime, id: &str, action: &str) -> Result<ActionResult, String> {
    check_instance_id(id)?;
    let instance = rt.store().instance(id).ok_or(INSTANCE_NOT_FOUND)?;
    let descriptor = find_connector(&instance.connector)?.descriptor();
    let action = check_action(descriptor, action)?;
    let pairing_code = if action == ActionId::Pair {
        if !rt.running_ids().iter().any(|running| running == id) {
            return Err(PAIR_NEEDS_RUNNING.to_string());
        }
        Some(rt.issue_pairing_code(id)?)
    } else {
        None
    };
    let (ctx, imp) = rt.context_for(id).await?;
    let ctx = match &pairing_code {
        Some(code) => ctx.with_pairing_code(code.clone()),
        None => ctx,
    };
    let outcome = with_timeout(ACTION_TIMEOUT, imp.connector().action(&ctx, action)).await.map_err(|e| {
        log::warn!("connettori {id}: azione {} non riuscita: {e}", action.as_str());
        format!("Azione non riuscita. {}", e.public_message())
    })?;
    restart_if_needs_auth(rt, id);
    Ok(ActionResult { message: outcome.message, detail: outcome.detail, pairing_code })
}

/// Verifica credenziali e rete (azione `test`, se il connettore la dichiara).
#[tauri::command]
pub async fn connector_test(app: AppHandle, id: String) -> Result<ActionResult, String> {
    check_instance_id(&id)?;
    execute(&app.state::<ConnectorsRuntime>(), &id, ActionId::Test.as_str()).await
}

/// Esegue un'azione dichiarata (`authorize`, `pair`, `test`). Con `pair` restituisce il codice di abbinamento.
#[tauri::command]
pub async fn connector_run_action(app: AppHandle, id: String, action: String) -> Result<ActionResult, String> {
    check_instance_id(&id)?;
    ActionId::parse(&action).ok_or("Azione sconosciuta.")?;
    execute(&app.state::<ConnectorsRuntime>(), &id, &action).await
}

#[cfg(test)]
mod tests;
