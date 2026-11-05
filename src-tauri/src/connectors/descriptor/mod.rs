//! Il descrittore di un connettore: dichiarato nel codice come `static`, e' tutto
//! cio' che l'interfaccia generica sa del connettore (campi, azioni) e il
//! riassunto revisionabile di cosa puo' fare (host di rete, capacita' verso
//! l'HUB). I tipi sono DTO verso il frontend (prefisso `Connector` nei
//! bindings). La validazione, pura, e' in `validate`. Vedi docs/CONNECTORS.MD §3.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "varianti e costanti (campi select, workspace, thread; nomi delle capacita') che nessun descrittore registrato usa ancora"
    )
)]

#[cfg(test)]
mod tests;
pub mod validate;

use serde::Serialize;
use ts_rs::TS;

/// Il tipo di connettore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "ConnectorKind")]
pub enum ConnectorKind {
    /// Solo in ingresso: porta dati esterni in un thread secondo una regola.
    Source,
    /// Bidirezionale: notifiche verso l'utente e messaggi dell'utente ai segretari.
    Contact,
}

/// Cio' che un connettore puo' chiedere all'HUB. Il runtime lo fa rispettare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "ConnectorHubCapability")]
pub enum HubCapability {
    /// Consegnare elementi esterni nei thread (`source`).
    DeliverMessages,
    /// Il campo `wake` delle regole diverso da `none` (`source`).
    WakeSecretaries,
    /// Ricevere le notifiche in uscita dell'HUB (`contact`).
    ReceiveNotices,
    /// Scrivere ai segretari come l'utente (`contact`).
    SendAsUser,
}

impl HubCapability {
    /// Nome sul filo e nei messaggi.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeliverMessages => "deliver_messages",
            Self::WakeSecretaries => "wake_secretaries",
            Self::ReceiveNotices => "receive_notices",
            Self::SendAsUser => "send_as_user",
        }
    }
}

/// Una voce di un campo `select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorSelectOption")]
pub struct SelectOption {
    pub value: &'static str,
    pub label: &'static str,
}

/// Il tipo di un campo e i suoi vincoli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, rename = "ConnectorFieldKind")]
pub enum FieldKind {
    /// Stringa su una riga, ripulita dagli spazi ai bordi.
    Text {
        max_len: usize,
        placeholder: &'static str,
    },
    /// Non sta mai in `config`: va nel portachiavi (slot = `key`).
    Secret,
    /// Intero fra `min` e `max`.
    Number {
        min: i64,
        max: i64,
    },
    Bool {
        default: bool,
    },
    /// Una delle `options`.
    Select {
        options: &'static [SelectOption],
    },
    /// L'id di un workspace esistente.
    Workspace,
    /// `{ channel, thread_id }` di un thread esistente.
    Thread,
}

/// Un campo della configurazione dell'istanza o del filtro di una regola.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorConfigField")]
pub struct ConfigField {
    /// `[a-z0-9_]`, unico nella sua lista.
    pub key: &'static str,
    pub label: &'static str,
    /// Testo d'aiuto, puo' citare un link della documentazione del servizio.
    pub help: &'static str,
    pub required: bool,
    pub kind: FieldKind,
}

/// Le azioni che l'interfaccia mostra come pulsanti.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename = "ConnectorActionId")]
pub enum ActionId {
    /// Consenso OAuth.
    Authorize,
    /// Codice di abbinamento di un account esterno (solo `contact`).
    Pair,
    /// Verifica di credenziali e rete.
    Test,
}

impl ActionId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authorize => "authorize",
            Self::Pair => "pair",
            Self::Test => "test",
        }
    }

    /// L'azione dal nome sul filo.
    pub fn parse(raw: &str) -> Option<Self> {
        [Self::Authorize, Self::Pair, Self::Test].into_iter().find(|a| a.as_str() == raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorAction")]
pub struct Action {
    pub id: ActionId,
    pub label: &'static str,
    pub help: &'static str,
}

/// Cosa il connettore puo' fare: la superficie da revisionare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorPermissions")]
pub struct Permissions {
    /// Host di rete ammessi (minuscoli, senza porta ne' schema): solo `https`.
    pub hosts: &'static [&'static str],
    pub hub: &'static [HubCapability],
}

/// Limiti facoltativi, piu' stretti di quelli del runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorDescriptorLimits")]
pub struct DescriptorLimits {
    /// Elementi consegnati per giro (al massimo `limits::ITEMS_PER_ROUND`).
    pub items_per_round: Option<usize>,
    /// Intervallo minimo di polling in secondi (almeno `limits::MIN_INTERVAL`).
    pub min_interval_secs: Option<u64>,
    /// Intervallo di polling usato quando la configurazione non ne imposta uno (almeno `limits::MIN_INTERVAL`).
    pub default_interval_secs: Option<u64>,
}

impl DescriptorLimits {
    /// Nessun limite piu' stretto di quelli del runtime.
    pub const NONE: Self = Self { items_per_round: None, min_interval_secs: None, default_interval_secs: None };
}

/// Il descrittore statico di un connettore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, rename = "ConnectorDescriptor")]
pub struct ConnectorDescriptor {
    /// Stabile, `[a-z0-9-]` senza trattini ai bordi; finisce nei file e negli autori dei messaggi.
    pub id: &'static str,
    pub kind: ConnectorKind,
    /// Italiano, per l'interfaccia.
    pub label: &'static str,
    pub description: &'static str,
    /// Intero >= 1: cresce quando cambia la forma di `config` o di `filter`, con
    /// una migrazione pura in `Connector::migrate_config`.
    pub version: u32,
    pub config_fields: &'static [ConfigField],
    /// Vuoto per i `contact`.
    pub filter_fields: &'static [ConfigField],
    pub actions: &'static [Action],
    pub permissions: Permissions,
    pub limits: DescriptorLimits,
    /// Slot del portachiavi che il connettore usa oltre ai campi `secret` (es. un
    /// refresh token ottenuto con `authorize`): non compaiono nell'interfaccia
    /// come campi.
    pub internal_secret_slots: &'static [&'static str],
}

impl ConnectorDescriptor {
    /// Tutti gli slot del portachiavi dell'istanza: i campi `secret` e gli slot interni.
    pub fn secret_slots(&self) -> Vec<&'static str> {
        let fields = self.config_fields.iter().filter(|f| matches!(f.kind, FieldKind::Secret)).map(|f| f.key);
        fields.chain(self.internal_secret_slots.iter().copied()).collect()
    }

    pub fn declares_slot(&self, slot: &str) -> bool {
        self.secret_slots().contains(&slot)
    }

    pub fn has_capability(&self, capability: HubCapability) -> bool {
        self.permissions.hub.contains(&capability)
    }

    pub fn has_action(&self, action: ActionId) -> bool {
        self.actions.iter().any(|a| a.id == action)
    }

    /// Il campo della configurazione con questa chiave.
    pub fn config_field(&self, key: &str) -> Option<&'static ConfigField> {
        self.config_fields.iter().find(|f| f.key == key)
    }
}
