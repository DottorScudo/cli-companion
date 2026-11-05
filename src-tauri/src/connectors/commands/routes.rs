//! Le regole delle fonti e i collegamenti dei canali di contatto. Le verifiche
//! pure sono in `checks`; qui si guarda il mondo (il canale esiste? il thread
//! esiste nell'HUB?) tramite `World`, che i test sostituiscono con un finto.

use super::checks::{
    apply_draft, check_instance_id, check_link_id, check_route, check_route_id, filter_routes, find_connector,
    new_route, parse_route_filter, source_of, thread_to_verify, RouteDraft, RouteInput, INSTANCE_NOT_FOUND,
    LINK_NOT_FOUND, ROUTE_NOT_FOUND,
};
use crate::connectors::ids::ROUTE_PREFIX;
use crate::connectors::model::{ConnectorLink, ConnectorRoute};
use crate::connectors::runtime::manager::ConnectorsRuntime;
use crate::hub::{thread_exists, Channel};
use crate::workspaces::WorkspaceStore;
use tauri::{AppHandle, Manager};

/// Cio' che serve sapere dell'HUB e dei workspace per validare una regola.
pub trait World {
    fn channel_exists(&self, channel: &Channel) -> bool;
    /// Errore se non si puo' sapere (HUB spento): non e' "no".
    fn thread_exists(&self, channel: &Channel, thread_id: &str) -> Result<bool, String>;
}

/// Il mondo vero: i workspace dell'app e l'API pubblica dell'HUB. Bloccante.
pub struct AppWorld(pub AppHandle);

impl World for AppWorld {
    fn channel_exists(&self, channel: &Channel) -> bool {
        match channel {
            Channel::General => true,
            Channel::Workspace(id) => self.0.state::<WorkspaceStore>().find(id).is_some(),
        }
    }

    fn thread_exists(&self, channel: &Channel, thread_id: &str) -> Result<bool, String> {
        thread_exists(&self.0, channel, thread_id)
    }
}

/// Le verifiche che guardano il mondo: il canale esiste, il thread (se serve) esiste.
fn check_world(world: &dyn World, old: Option<&ConnectorRoute>, draft: &RouteDraft) -> Result<(), String> {
    if !world.channel_exists(&draft.channel) {
        return Err("Il canale scelto non esiste più.".to_string());
    }
    if let Some(thread_id) = thread_to_verify(old, draft) {
        if !world.thread_exists(&draft.channel, thread_id)? {
            return Err("Il thread scelto non esiste più.".to_string());
        }
    }
    Ok(())
}

/// Crea una regola di una fonte. Bloccante.
pub fn create_route(
    rt: &ConnectorsRuntime,
    world: &dyn World,
    instance: &str,
    input: RouteInput,
) -> Result<ConnectorRoute, String> {
    check_instance_id(instance)?;
    let owner = rt.store().instance(instance).ok_or(INSTANCE_NOT_FOUND)?;
    let source = source_of(find_connector(&owner.connector)?)?;
    let draft = check_route(source, input)?;
    rt.store().read(|f| f.check_route_capacity(instance))?;
    check_world(world, None, &draft)?;
    let route = new_route(rt.new_id(ROUTE_PREFIX)?, instance, draft, rt.now_ms()?);
    let (to_add, owner_id) = (route.clone(), instance.to_string());
    rt.mutate(move |file| {
        file.instance(&owner_id).ok_or(INSTANCE_NOT_FOUND)?;
        file.check_route_capacity(&owner_id)?;
        file.routes.push(to_add);
        Ok(())
    })?;
    Ok(route)
}

/// Modifica una regola (l'istanza non cambia). Bloccante.
pub fn update_route(
    rt: &ConnectorsRuntime,
    world: &dyn World,
    id: &str,
    input: RouteInput,
) -> Result<ConnectorRoute, String> {
    check_route_id(id)?;
    let old = rt.store().read(|f| f.routes.iter().find(|r| r.id == id).cloned()).ok_or(ROUTE_NOT_FOUND)?;
    let owner = rt.store().instance(&old.instance).ok_or(INSTANCE_NOT_FOUND)?;
    let source = source_of(find_connector(&owner.connector)?)?;
    let draft = check_route(source, input)?;
    check_world(world, Some(&old), &draft)?;
    let (now, route_id) = (rt.now_ms()?, id.to_string());
    let saved = rt.mutate(move |file| {
        let route = file.routes.iter_mut().find(|r| r.id == route_id).ok_or(ROUTE_NOT_FOUND)?;
        apply_draft(route, draft, now);
        Ok(())
    })?;
    saved.routes.into_iter().find(|r| r.id == id).ok_or_else(|| ROUTE_NOT_FOUND.to_string())
}

pub fn delete_route(rt: &ConnectorsRuntime, id: &str) -> Result<(), String> {
    check_route_id(id)?;
    let route_id = id.to_string();
    rt.mutate(move |file| {
        let before = file.routes.len();
        file.routes.retain(|r| r.id != route_id);
        if file.routes.len() == before {
            return Err(ROUTE_NOT_FOUND.to_string());
        }
        Ok(())
    })
    .map(|_| ())
}

pub fn list_links(rt: &ConnectorsRuntime, instance: &str) -> Result<Vec<ConnectorLink>, String> {
    check_instance_id(instance)?;
    Ok(rt.store().read(|f| f.links_of(instance).cloned().collect()))
}

pub fn delete_link(rt: &ConnectorsRuntime, id: &str) -> Result<(), String> {
    check_link_id(id)?;
    let link_id = id.to_string();
    rt.mutate(move |file| {
        let before = file.links.len();
        file.links.retain(|l| l.id != link_id);
        if file.links.len() == before {
            return Err(LINK_NOT_FOUND.to_string());
        }
        Ok(())
    })
    .map(|_| ())
}

/// Le regole, filtrabili per canale e per thread (le regole che puntano a quel thread).
#[tauri::command]
pub async fn connector_list_routes(
    app: AppHandle,
    channel: Option<String>,
    thread_id: Option<String>,
) -> Result<Vec<ConnectorRoute>, String> {
    let filter = parse_route_filter(channel.as_deref(), thread_id.as_deref())?;
    crate::blocking(move || {
        let routes = app.state::<ConnectorsRuntime>().snapshot().routes;
        Ok(filter_routes(&routes, &filter))
    })
    .await
}

/// Crea una regola di una fonte: filtro, canale, thread di destinazione e risveglio.
#[tauri::command]
pub async fn connector_create_route(
    app: AppHandle,
    instance: String,
    route: RouteInput,
) -> Result<ConnectorRoute, String> {
    check_instance_id(&instance)?;
    let input = route;
    crate::blocking(move || create_route(&app.state::<ConnectorsRuntime>(), &AppWorld(app.clone()), &instance, input))
        .await
}

#[tauri::command]
pub async fn connector_update_route(app: AppHandle, id: String, route: RouteInput) -> Result<ConnectorRoute, String> {
    check_route_id(&id)?;
    let input = route;
    crate::blocking(move || update_route(&app.state::<ConnectorsRuntime>(), &AppWorld(app.clone()), &id, input)).await
}

#[tauri::command]
pub async fn connector_delete_route(app: AppHandle, id: String) -> Result<(), String> {
    check_route_id(&id)?;
    crate::blocking(move || delete_route(&app.state::<ConnectorsRuntime>(), &id)).await
}

/// Gli account esterni collegati a un'istanza di contatto.
#[tauri::command]
pub async fn connector_list_links(app: AppHandle, instance: String) -> Result<Vec<ConnectorLink>, String> {
    check_instance_id(&instance)?;
    crate::blocking(move || list_links(&app.state::<ConnectorsRuntime>(), &instance)).await
}

#[tauri::command]
pub async fn connector_delete_link(app: AppHandle, id: String) -> Result<(), String> {
    check_link_id(&id)?;
    crate::blocking(move || delete_link(&app.state::<ConnectorsRuntime>(), &id)).await
}

#[cfg(test)]
mod tests;
