//! Migrazione e validazione della configurazione di un'istanza al caricamento.
//! Se `config_version` e' piu' vecchia della `version` del descrittore, la
//! configurazione e i filtri delle regole dell'istanza (migrano con la stessa
//! versione) passano di un passo alla volta per `Connector::migrate_config`. Tutto
//! o niente: se un passo fallisce l'istanza resta com'era (nulla si perde) e il
//! runtime la mostra in `error`. Il risultato vive in memoria: si scrive su disco
//! al prossimo salvataggio. Puro: niente I/O.

use crate::connectors::contracts::Connector;
use crate::connectors::descriptor::validate::validate_values;
use crate::connectors::model::{ConnectorInstance, ConnectorsFile};

/// Porta l'istanza `id` alla versione corrente del connettore. `Ok(true)` se ha migrato qualcosa.
pub fn migrate_instance(file: &mut ConnectorsFile, id: &str, connector: &dyn Connector) -> Result<bool, String> {
    let target = connector.descriptor().version;
    let Some(instance) = file.instance(id) else { return Err("istanza inesistente".to_string()) };
    let from = instance.config_version;
    if from == target {
        return Ok(false);
    }
    if from > target {
        return Err(format!("configurazione di una versione più recente ({from}) di quella supportata ({target})"));
    }
    let step = |mut value: serde_json::Value| -> Result<serde_json::Value, String> {
        for version in from..target {
            value = connector.migrate_config(version, value)?;
        }
        Ok(value)
    };
    let config = step(instance.config.clone())?;
    let mut filters = Vec::new();
    for route in file.routes_of(id) {
        filters.push((route.id.clone(), step(route.filter.clone())?));
    }
    if let Some(instance) = file.instance_mut(id) {
        instance.config = config;
        instance.config_version = target;
    }
    for (route_id, filter) in filters {
        if let Some(route) = file.routes.iter_mut().find(|r| r.id == route_id) {
            route.filter = filter;
        }
    }
    Ok(true)
}

/// La configurazione (gia' migrata) e' valida per il connettore? Prima il
/// controllo generico contro i campi del descrittore, poi quello semantico.
pub fn validate_instance(connector: &dyn Connector, instance: &ConnectorInstance) -> Result<(), String> {
    validate_values(connector.descriptor().config_fields, &instance.config)?;
    connector.validate_config(&instance.config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::model::{RouteThread, RouteWake};
    use crate::connectors::testing::{instance, new_contact, new_source, route, INSTANCE};
    use serde_json::json;

    fn file_with(version: u32, connector: &str) -> ConnectorsFile {
        ConnectorsFile {
            instances: vec![instance(INSTANCE, connector, json!({"mailbox": "casella"}), version)],
            routes: vec![route("rte-1-1", RouteThread::NewPerItem, RouteWake::None)],
            links: Vec::new(),
        }
    }

    #[test]
    fn an_old_config_is_migrated_step_by_step_with_the_filters() {
        let mut file = file_with(1, "fake-source");
        assert_eq!(migrate_instance(&mut file, INSTANCE, new_source()), Ok(true));
        let migrated = file.instance(INSTANCE).unwrap();
        assert_eq!(migrated.config_version, 2);
        assert_eq!(migrated.config["interval_secs"], 60);
        assert_eq!(file.routes[0].filter["interval_secs"], 60, "il filtro migra con la stessa funzione");
    }

    #[test]
    fn a_current_config_is_left_alone() {
        let mut file = file_with(2, "fake-source");
        let before = file.clone();
        assert_eq!(migrate_instance(&mut file, INSTANCE, new_source()), Ok(false));
        assert_eq!(file, before);
    }

    #[test]
    fn a_failed_migration_keeps_the_original_everything() {
        let mut file = file_with(0, "fake-contact");
        let before = file.clone();
        assert!(migrate_instance(&mut file, INSTANCE, new_contact()).is_err());
        assert_eq!(file, before);
        // Una versione piu' nuova di quella supportata non si tocca.
        let mut newer = file_with(9, "fake-source");
        let before = newer.clone();
        assert!(migrate_instance(&mut newer, INSTANCE, new_source()).unwrap_err().contains("più recente"));
        assert_eq!(newer, before);
        assert!(migrate_instance(&mut newer, "con-9-9", new_source()).is_err());
    }

    #[test]
    fn validation_is_generic_then_semantic() {
        let ok = instance(INSTANCE, "fake-source", json!({"mailbox": "casella", "interval_secs": 120}), 2);
        assert!(validate_instance(new_source(), &ok).is_ok());
        let unknown_field = instance(INSTANCE, "fake-source", json!({"altro": 1}), 2);
        assert!(validate_instance(new_source(), &unknown_field).is_err());
        let semantic = instance(INSTANCE, "fake-source", json!({"mailbox": "vietata"}), 2);
        assert_eq!(validate_instance(new_source(), &semantic), Err("casella non ammessa".to_string()));
    }
}
