//! Attrezzi dei test dei comandi: un runtime installato con finti (HUB, orologio,
//! segreti, stato in memoria), i connettori finti registrati per il thread del
//! test e un `World` finto.

use super::routes::World;
use crate::connectors::contracts::ConnectorImpl;
use crate::connectors::model::ConnectorsFile;
use crate::connectors::registry::test_registry::Registered;
use crate::connectors::runtime::manager::{ConnectorsRuntime, Platform};
use crate::connectors::testing::{
    instance, new_contact, new_source, FakeClock, FakeHub, MemoryStates, ParkedSleeper, INSTANCE,
};
use crate::hub::Channel;
use crate::secrets::{MemorySecrets, SecretStore};
use serde_json::json;
use std::sync::Arc;

pub struct Fixture {
    pub rt: ConnectorsRuntime,
    pub secrets: Arc<dyn SecretStore>,
    _registered: Registered,
}

pub fn fixture_with(file: ConnectorsFile, secrets: Arc<dyn SecretStore>) -> Fixture {
    let (source, contact) = (new_source(), new_contact());
    let registered = Registered::new(vec![ConnectorImpl::Source(source), ConnectorImpl::Contact(contact)]);
    let rt = ConnectorsRuntime::default();
    rt.store().replace(file);
    rt.install(Platform {
        hub: Arc::new(FakeHub::default()),
        clock: Arc::new(FakeClock::new(1_700_000_000_000)),
        sleeper: Arc::new(ParkedSleeper),
        states: Arc::new(MemoryStates::default()),
        secrets: secrets.clone(),
    });
    Fixture { rt, secrets, _registered: registered }
}

pub fn fixture(file: ConnectorsFile) -> Fixture {
    fixture_with(file, Arc::new(MemorySecrets::default()))
}

/// Un file con una fonte (`INSTANCE`) spenta.
pub fn source_file() -> ConnectorsFile {
    let mut inst = instance(INSTANCE, "fake-source", json!({}), 2);
    inst.enabled = false;
    ConnectorsFile { instances: vec![inst], routes: Vec::new(), links: Vec::new() }
}

/// Un file con un canale di contatto (`INSTANCE`) abilitato.
pub fn contact_file() -> ConnectorsFile {
    ConnectorsFile {
        instances: vec![instance(INSTANCE, "fake-contact", json!({}), 1)],
        routes: Vec::new(),
        links: Vec::new(),
    }
}

/// Workspace e thread finti; `hub_on = false` fa rispondere errore alla ricerca di un thread.
#[derive(Default)]
pub struct FakeWorld {
    pub workspaces: Vec<String>,
    pub threads: Vec<(String, String)>,
    pub hub_off: bool,
}

impl World for FakeWorld {
    fn channel_exists(&self, channel: &Channel) -> bool {
        match channel {
            Channel::General => true,
            Channel::Workspace(id) => self.workspaces.contains(id),
        }
    }

    fn thread_exists(&self, channel: &Channel, thread_id: &str) -> Result<bool, String> {
        if self.hub_off {
            return Err("L'Agents HUB è spento.".to_string());
        }
        Ok(self.threads.iter().any(|(c, t)| c == channel.as_str() && t == thread_id))
    }
}
