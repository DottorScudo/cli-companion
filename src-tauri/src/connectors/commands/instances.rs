//! Le istanze: elenco, creazione, modifica, abilitazione, eliminazione, segreti.
//! Le funzioni `*_instance`, `set_secret` e `clear_secret` lavorano sul
//! `ConnectorsRuntime` e sono bloccanti (salvano su disco, toccano il
//! portachiavi); i comandi Tauri validano per primi e le eseguono in
//! `crate::blocking`.

use super::checks::{
    check_config, check_instance_id, check_label, check_slot, find_connector, INSTANCE_NOT_FOUND, NOT_AVAILABLE,
};
use super::view::{build_view, DeleteResult, InstanceView};
use crate::connectors::descriptor::validate::valid_connector_id;
use crate::connectors::descriptor::ConnectorDescriptor;
use crate::connectors::files::StateFile;
use crate::connectors::ids::INSTANCE_PREFIX;
use crate::connectors::model::{ConnectorInstance, ConnectorNotices};
use crate::connectors::registry;
use crate::connectors::runtime::health::{ConnectorStatus, HealthState};
use crate::connectors::runtime::manager::ConnectorsRuntime;
use crate::connectors::secrets::validate_value;
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Crea un'istanza spenta. La configurazione e' validata (e senza segreti: si impostano con `set_secret`).
pub fn create_instance(
    rt: &ConnectorsRuntime,
    connector: &str,
    label: &str,
    config: Value,
) -> Result<ConnectorInstance, String> {
    if !valid_connector_id(connector) {
        return Err(NOT_AVAILABLE.to_string());
    }
    let imp = find_connector(connector)?;
    let label = check_label(label)?;
    let config = check_config(imp.connector(), config)?;
    rt.store().read(|f| f.check_instance_capacity())?;
    let now = rt.now_ms()?;
    let created = ConnectorInstance {
        id: rt.new_id(INSTANCE_PREFIX)?,
        connector: connector.to_string(),
        label,
        enabled: false,
        config,
        config_version: imp.descriptor().version,
        notices: ConnectorNotices::default(),
        created_at: now,
        updated_at: now,
    };
    let to_add = created.clone();
    rt.mutate(move |file| {
        file.check_instance_capacity()?;
        file.instances.push(to_add);
        Ok(())
    })?;
    Ok(created)
}

/// Cambia etichetta e configurazione. Se la configurazione cambia l'istanza
/// riparte (cosi' esce da `needs_auth`/`error`). Una configurazione salvata con
/// un'altra versione del descrittore non si modifica: i filtri delle regole
/// sono legati a quella versione.
pub fn update_instance(
    rt: &ConnectorsRuntime,
    id: &str,
    label: &str,
    config: Value,
) -> Result<ConnectorInstance, String> {
    check_instance_id(id)?;
    let label = check_label(label)?;
    let current = rt.store().instance(id).ok_or(INSTANCE_NOT_FOUND)?;
    let imp = find_connector(&current.connector)?;
    let version = imp.descriptor().version;
    if current.config_version != version {
        return Err("La configurazione è di una versione precedente e non è stata aggiornata: non si può modificare."
            .to_string());
    }
    let config = check_config(imp.connector(), config)?;
    let restart = current.config != config;
    let now = rt.now_ms()?;
    let (id_owned, new_config) = (id.to_string(), config);
    let saved = rt.mutate(move |file| {
        let instance = file.instance_mut(&id_owned).ok_or(INSTANCE_NOT_FOUND)?;
        instance.label = label;
        instance.config = new_config;
        instance.updated_at = now;
        Ok(())
    })?;
    if restart {
        rt.restart_instance(id);
    }
    saved.instance(id).cloned().ok_or_else(|| INSTANCE_NOT_FOUND.to_string())
}

/// Abilita o disabilita. Un connettore non compilato in questa versione non si abilita.
pub fn set_enabled(rt: &ConnectorsRuntime, id: &str, enabled: bool) -> Result<ConnectorInstance, String> {
    check_instance_id(id)?;
    let current = rt.store().instance(id).ok_or(INSTANCE_NOT_FOUND)?;
    if enabled {
        find_connector(&current.connector)?;
    }
    let now = rt.now_ms()?;
    let id_owned = id.to_string();
    let saved = rt.mutate(move |file| {
        let instance = file.instance_mut(&id_owned).ok_or(INSTANCE_NOT_FOUND)?;
        if instance.enabled != enabled {
            instance.enabled = enabled;
            instance.updated_at = now;
        }
        Ok(())
    })?;
    saved.instance(id).cloned().ok_or_else(|| INSTANCE_NOT_FOUND.to_string())
}

/// Elimina l'istanza con le sue regole e i suoi collegamenti, ferma il task e
/// toglie stato e segreti. `delete_state` cancella il file di stato. Un errore
/// del portachiavi o del file di stato non annulla l'eliminazione: si riporta in `warning`.
pub fn delete_instance(
    rt: &ConnectorsRuntime,
    id: &str,
    delete_state: impl FnOnce(&str) -> Result<(), String>,
) -> Result<DeleteResult, String> {
    check_instance_id(id)?;
    let current = rt.store().instance(id).ok_or(INSTANCE_NOT_FOUND)?;
    let slots: Vec<&'static str> =
        registry::find(&current.connector).map(|c| c.descriptor().secret_slots()).unwrap_or_default();
    let id_owned = id.to_string();
    rt.mutate(move |file| {
        file.remove_instance(&id_owned);
        Ok(())
    })?;
    rt.forget_instance(id);
    let mut warnings = Vec::new();
    if let Err(e) = delete_state(id) {
        log::error!("connettori {id}: {e}");
        warnings.push("lo stato del connettore non è stato cancellato".to_string());
    }
    match rt.instance_secrets(id).and_then(|secrets| secrets.delete_all(&slots)) {
        Ok(()) => {}
        Err(e) => {
            log::error!("connettori {id}: {e}");
            warnings.push("alcuni segreti non sono stati rimossi dal portachiavi del sistema".to_string());
        }
    }
    let warning = (!warnings.is_empty()).then(|| format!("Connettore eliminato, ma {}.", warnings.join(" e ")));
    Ok(DeleteResult { warning })
}

/// Il descrittore e l'istanza di un comando sui segreti.
fn secret_target(
    rt: &ConnectorsRuntime,
    id: &str,
    slot: &str,
) -> Result<(ConnectorInstance, &'static ConnectorDescriptor), String> {
    check_instance_id(id)?;
    let instance = rt.store().instance(id).ok_or(INSTANCE_NOT_FOUND)?;
    let descriptor = find_connector(&instance.connector)?.descriptor();
    check_slot(descriptor, slot)?;
    Ok((instance, descriptor))
}

/// Imposta il segreto di uno slot dichiarato. Se l'istanza era ferma per
/// credenziali rifiutate (`needs_auth`) riparte col valore nuovo.
pub fn set_secret(rt: &ConnectorsRuntime, id: &str, slot: &str, value: &str) -> Result<ConnectorInstance, String> {
    let (instance, _) = secret_target(rt, id, slot)?;
    rt.instance_secrets(id)?.set(slot, value)?;
    restart_if_needs_auth(rt, id);
    Ok(instance)
}

/// Toglie il segreto di uno slot dichiarato.
pub fn clear_secret(rt: &ConnectorsRuntime, id: &str, slot: &str) -> Result<ConnectorInstance, String> {
    let (instance, _) = secret_target(rt, id, slot)?;
    rt.instance_secrets(id)?.clear(slot)?;
    Ok(instance)
}

/// Fa ripartire l'istanza se e' ferma in `needs_auth`.
pub fn restart_if_needs_auth(rt: &ConnectorsRuntime, id: &str) {
    if rt.status_registry().get(id).is_some_and(|s| s.state == HealthState::NeedsAuth) {
        rt.restart_instance(id);
    }
}

/// Descrittori di tutti i connettori compilati.
#[tauri::command]
pub async fn connector_list_descriptors() -> Result<Vec<ConnectorDescriptor>, String> {
    Ok(registry::all().iter().map(|c| *c.descriptor()).collect())
}

#[tauri::command]
pub async fn connector_list_instances(app: AppHandle) -> Result<Vec<InstanceView>, String> {
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        Ok(rt.snapshot().instances.into_iter().map(|instance| build_view(&rt, instance)).collect())
    })
    .await
}

/// Lo stato di salute di tutte le istanze (lo stesso elenco dell'evento `connector-status`).
#[tauri::command]
pub async fn connector_status(app: AppHandle) -> Result<Vec<ConnectorStatus>, String> {
    Ok(app.state::<ConnectorsRuntime>().statuses())
}

/// Crea un'istanza disabilitata; i segreti si impostano dopo con `connector_set_secret`.
#[tauri::command]
pub async fn connector_create_instance(
    app: AppHandle,
    connector: String,
    label: String,
    config: Value,
) -> Result<InstanceView, String> {
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let created = create_instance(&rt, &connector, &label, config)?;
        Ok(build_view(&rt, created))
    })
    .await
}

#[tauri::command]
pub async fn connector_update_instance(
    app: AppHandle,
    id: String,
    label: String,
    config: Value,
) -> Result<InstanceView, String> {
    check_instance_id(&id)?;
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let updated = update_instance(&rt, &id, &label, config)?;
        Ok(build_view(&rt, updated))
    })
    .await
}

#[tauri::command]
pub async fn connector_delete_instance(app: AppHandle, id: String) -> Result<DeleteResult, String> {
    check_instance_id(&id)?;
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let dir = crate::store::data_dir(&app)?;
        delete_instance(&rt, &id, |id| StateFile::open(&dir, id)?.delete())
    })
    .await
}

#[tauri::command]
pub async fn connector_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<InstanceView, String> {
    check_instance_id(&id)?;
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let updated = set_enabled(&rt, &id, enabled)?;
        Ok(build_view(&rt, updated))
    })
    .await
}

/// Salva un segreto nel portachiavi del sistema; non lo restituisce e non lo scrive nel log.
#[tauri::command]
pub async fn connector_set_secret(
    app: AppHandle,
    id: String,
    slot: String,
    value: String,
) -> Result<InstanceView, String> {
    check_instance_id(&id)?;
    validate_value(&value)?;
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let instance = set_secret(&rt, &id, &slot, &value)?;
        Ok(build_view(&rt, instance))
    })
    .await
}

#[tauri::command]
pub async fn connector_clear_secret(app: AppHandle, id: String, slot: String) -> Result<InstanceView, String> {
    check_instance_id(&id)?;
    crate::blocking(move || {
        let rt = app.state::<ConnectorsRuntime>();
        let instance = clear_secret(&rt, &id, &slot)?;
        Ok(build_view(&rt, instance))
    })
    .await
}

#[cfg(test)]
mod tests;
