//! Il modello dei connettori: istanze, regole (fonti), collegamenti (canali di
//! contatto) e il file `connectors.json` che li contiene. Sono DTO verso il
//! frontend (prefisso `Connector` nei bindings) e anche il formato su disco.
//! Mai segreti qui: stanno nel portachiavi (`secrets`).
//!
//! Compatibilita': i campi facoltativi hanno `#[serde(default)]`. Una istanza il
//! cui connettore non e' nel registro resta com'e' (configurazione compresa): il
//! modello non ne sa nulla, e' il runtime a mostrarla come non disponibile.
//! I filtri delle regole migrano con la stessa `config_version` dell'istanza.

use super::descriptor::validate::valid_connector_id;
use super::ids::{valid_instance_id, valid_link_id, valid_route_id};
use super::limits::{LABEL_MAX, MAX_INSTANCES, MAX_LINKS_PER_INSTANCE, MAX_ROUTES_PER_INSTANCE};
use crate::hub::ingress::ExternalWake;
use crate::hub::{valid_ref_id, Channel, BOT_NAME_MAX};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[cfg(test)]
mod tests;

/// Lunghezza massima di un id esterno (account, chat) di un collegamento.
pub const EXTERNAL_REF_MAX: usize = 128;
/// Lunghezza massima del nome mostrato di un account esterno.
pub const DISPLAY_NAME_MAX: usize = 60;
/// Il motivo con cui una regola si disabilita da sola se il suo thread sparisce.
pub const THREAD_GONE: &str = "thread non più disponibile";

/// Quali notifiche dell'HUB un canale di contatto riceve: l'interruttore generale
/// e uno per tipo. Indipendente dalle notifiche di sistema dell'app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export, rename = "ConnectorNotices")]
pub struct ConnectorNotices {
    pub all: bool,
    pub replies: bool,
    pub questions: bool,
    pub failures: bool,
    pub attention: bool,
    pub proposals: bool,
    pub conclusions: bool,
    pub sources: bool,
}

impl Default for ConnectorNotices {
    fn default() -> Self {
        Self {
            all: true,
            replies: true,
            questions: true,
            failures: true,
            attention: true,
            proposals: true,
            conclusions: true,
            sources: true,
        }
    }
}

/// Un'istanza di un connettore, creata disabilitata dall'utente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, rename = "ConnectorInstance")]
pub struct ConnectorInstance {
    /// `con-<ms>-<n>`.
    pub id: String,
    /// L'id del descrittore (`connectors/registry`); puo' non essere (piu') registrato.
    pub connector: String,
    pub label: String,
    pub enabled: bool,
    /// Senza segreti: validata contro i campi del descrittore.
    #[ts(type = "Record<string, unknown>")]
    pub config: Value,
    /// La `version` del descrittore con cui `config` (e i filtri delle regole) e' stata salvata.
    pub config_version: u32,
    #[serde(default)]
    pub notices: ConnectorNotices,
    pub created_at: i64,
    pub updated_at: i64,
}

/// In quale thread finisce un elemento di una fonte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export, rename = "ConnectorRouteThread")]
pub enum RouteThread {
    /// Un thread scelto dall'utente: gli elementi vanno tutti li'.
    Existing { thread_id: String },
    /// Un thread nuovo per elemento, intitolato col titolo.
    NewPerItem,
    /// La chiave di conversazione dell'elemento sceglie il thread (mappa nello stato).
    PerConversation,
}

/// Chi svegliare alla consegna. Sul filo e su disco e' una stringa:
/// `none`, `owner` o `secretary:<nome>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export, rename = "ConnectorRouteWake", type = "string")]
pub enum RouteWake {
    None,
    Owner,
    Secretary(String),
}

impl RouteWake {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "none" => Ok(Self::None),
            "owner" => Ok(Self::Owner),
            _ => {
                let name = raw.strip_prefix("secretary:").ok_or("risveglio non valido")?;
                let chars = name.chars().count();
                if chars == 0 || chars > BOT_NAME_MAX || name != name.trim() || name.chars().any(char::is_control) {
                    return Err("nome del segretario non valido".to_string());
                }
                Ok(Self::Secretary(name.to_string()))
            }
        }
    }

    /// Il risveglio per `hub::ingress`.
    pub fn to_external(&self) -> ExternalWake {
        match self {
            Self::None => ExternalWake::None,
            Self::Owner => ExternalWake::Owner,
            Self::Secretary(name) => ExternalWake::Secretary(name.clone()),
        }
    }

    pub fn wakes(&self) -> bool {
        *self != Self::None
    }
}

impl TryFrom<String> for RouteWake {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, String> {
        Self::parse(&raw)
    }
}

impl From<RouteWake> for String {
    fn from(wake: RouteWake) -> Self {
        match wake {
            RouteWake::None => "none".to_string(),
            RouteWake::Owner => "owner".to_string(),
            RouteWake::Secretary(name) => format!("secretary:{name}"),
        }
    }
}

/// Una regola di una fonte: cosa prendere (filtro) e dove metterlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, rename = "ConnectorRoute")]
pub struct ConnectorRoute {
    /// `rte-<ms>-<n>`.
    pub id: String,
    pub instance: String,
    pub label: String,
    #[ts(type = "Record<string, unknown>")]
    pub filter: Value,
    #[ts(type = "string")]
    pub channel: Channel,
    pub thread: RouteThread,
    pub wake: RouteWake,
    pub enabled: bool,
    /// Perche' la regola si e' disabilitata da sola (es. `THREAD_GONE`).
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Il thread corrente di un collegamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "ConnectorLinkThread")]
pub struct LinkThread {
    #[ts(type = "string")]
    pub channel: Channel,
    pub thread_id: String,
}

/// Un account esterno abbinato all'utente (canale di contatto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "ConnectorLink")]
pub struct ConnectorLink {
    /// `lnk-<ms>-<n>`.
    pub id: String,
    pub instance: String,
    /// L'id dell'utente esterno: stringa opaca.
    pub external_account: String,
    /// La chat privata in cui scrivere.
    pub external_chat: String,
    /// Il nome esterno ripulito, solo per l'interfaccia.
    pub display_name: String,
    #[serde(default)]
    pub current: Option<LinkThread>,
    /// Il canale scelto con `/ws`: li' `/new` apre i thread. Assente = quello del thread corrente, o il Generale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "string")]
    pub channel: Option<Channel>,
    #[serde(default)]
    pub muted_until: Option<i64>,
    pub created_at: i64,
}

/// Il contenuto di `connectors.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnectorsFile {
    #[serde(default)]
    pub instances: Vec<ConnectorInstance>,
    #[serde(default)]
    pub routes: Vec<ConnectorRoute>,
    #[serde(default)]
    pub links: Vec<ConnectorLink>,
}

/// Cosa ha tolto `ConnectorsFile::forget_channel`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForgottenChannel {
    /// `(istanza, regola)` delle regole eliminate.
    pub routes: Vec<(String, String)>,
    /// Collegamenti il cui thread corrente o canale scelto puntava al canale (ora `None`).
    pub links_reset: usize,
}

/// L'etichetta e' valida (1..=LABEL_MAX caratteri, senza spazi ai bordi ne' di controllo)?
pub fn label_ok(label: &str) -> bool {
    let chars = label.trim().chars().count();
    label == label.trim() && (1..=LABEL_MAX).contains(&chars) && !label.chars().any(char::is_control)
}

/// Controlli strutturali di una voce letta dal file. Non guardano il registro:
/// una istanza di un connettore sconosciuto e' valida.
impl ConnectorInstance {
    pub fn check(&self) -> Result<(), String> {
        if !valid_instance_id(&self.id) {
            return Err("id istanza non valido".to_string());
        }
        if !valid_connector_id(&self.connector) {
            return Err("id del connettore non valido".to_string());
        }
        if !label_ok(&self.label) {
            return Err("etichetta non valida".to_string());
        }
        if !self.config.is_object() {
            return Err("configurazione non valida".to_string());
        }
        Ok(())
    }
}

impl ConnectorRoute {
    pub fn check(&self) -> Result<(), String> {
        if !valid_route_id(&self.id) || !valid_instance_id(&self.instance) {
            return Err("id regola o istanza non valido".to_string());
        }
        if !label_ok(&self.label) || !self.filter.is_object() {
            return Err("etichetta o filtro non validi".to_string());
        }
        match &self.thread {
            RouteThread::Existing { thread_id } if !valid_ref_id(thread_id) => {
                Err("id del thread non valido".to_string())
            }
            _ => Ok(()),
        }
    }
}

impl ConnectorLink {
    pub fn check(&self) -> Result<(), String> {
        let ref_ok =
            |s: &str| !s.is_empty() && s.chars().count() <= EXTERNAL_REF_MAX && !s.chars().any(char::is_control);
        if !valid_link_id(&self.id) || !valid_instance_id(&self.instance) {
            return Err("id collegamento o istanza non valido".to_string());
        }
        if !ref_ok(&self.external_account) || !ref_ok(&self.external_chat) {
            return Err("account o chat esterni non validi".to_string());
        }
        if self.display_name.chars().count() > DISPLAY_NAME_MAX || self.display_name.chars().any(char::is_control) {
            return Err("nome mostrato non valido".to_string());
        }
        match &self.current {
            Some(current) if !valid_ref_id(&current.thread_id) => Err("thread corrente non valido".to_string()),
            _ => Ok(()),
        }
    }
}

impl ConnectorsFile {
    pub fn instance(&self, id: &str) -> Option<&ConnectorInstance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn instance_mut(&mut self, id: &str) -> Option<&mut ConnectorInstance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    pub fn routes_of<'a>(&'a self, instance: &'a str) -> impl Iterator<Item = &'a ConnectorRoute> {
        self.routes.iter().filter(move |r| r.instance == instance)
    }

    pub fn links_of<'a>(&'a self, instance: &'a str) -> impl Iterator<Item = &'a ConnectorLink> {
        self.links.iter().filter(move |l| l.instance == instance)
    }

    /// C'e' posto per un'altra istanza?
    pub fn check_instance_capacity(&self) -> Result<(), String> {
        if self.instances.len() >= MAX_INSTANCES {
            return Err(format!("hai raggiunto il limite di {MAX_INSTANCES} connettori configurati"));
        }
        Ok(())
    }

    /// C'e' posto per un'altra regola in `instance`?
    pub fn check_route_capacity(&self, instance: &str) -> Result<(), String> {
        if self.routes_of(instance).count() >= MAX_ROUTES_PER_INSTANCE {
            return Err(format!("una istanza può avere al massimo {MAX_ROUTES_PER_INSTANCE} regole"));
        }
        Ok(())
    }

    /// C'e' posto per un altro collegamento in `instance`?
    pub fn check_link_capacity(&self, instance: &str) -> Result<(), String> {
        if self.links_of(instance).count() >= MAX_LINKS_PER_INSTANCE {
            return Err(format!("una istanza può avere al massimo {MAX_LINKS_PER_INSTANCE} account collegati"));
        }
        Ok(())
    }

    /// Toglie l'istanza con le sue regole e i suoi collegamenti. `false` se non c'era.
    pub fn remove_instance(&mut self, id: &str) -> bool {
        let before = self.instances.len();
        self.instances.retain(|i| i.id != id);
        self.routes.retain(|r| r.instance != id);
        self.links.retain(|l| l.instance != id);
        self.instances.len() != before
    }

    /// Il canale e' sparito (workspace rimosso): via le regole con quel canale e il
    /// thread corrente e il canale scelto dei collegamenti che puntavano li'.
    pub fn forget_channel(&mut self, channel: &Channel) -> ForgottenChannel {
        let mut forgotten = ForgottenChannel::default();
        self.routes.retain(|r| {
            let gone = r.channel == *channel;
            if gone {
                forgotten.routes.push((r.instance.clone(), r.id.clone()));
            }
            !gone
        });
        for link in &mut self.links {
            let had_current = link.current.as_ref().is_some_and(|c| c.channel == *channel);
            let had_channel = link.channel.as_ref() == Some(channel);
            if had_current {
                link.current = None;
            }
            if had_channel {
                link.channel = None;
            }
            if had_current || had_channel {
                forgotten.links_reset += 1;
            }
        }
        forgotten
    }
}
