//! Il descrittore di Gmail: campi, filtro, azioni, permessi e limiti. Guida: docs/CONNECTORS.MD §13.

use super::keys::{
    ACCOUNTS_HOST, API_HOST, CLIENT_ID_KEY, CLIENT_SECRET_SLOT, ID, INTERVAL_KEY, OAUTH_HOST, QUERY_KEY,
    REFRESH_TOKEN_SLOT,
};
use super::validate::{CLIENT_ID_MAX, QUERY_MAX};
use crate::connectors::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, DescriptorLimits, FieldKind, HubCapability,
    Permissions,
};

/// Intervallo fra due giri predefinito, in secondi (2 minuti).
const INTERVAL_DEFAULT_SECS: u64 = 120;
/// Estremi dell'intervallo configurabile, in minuti.
const INTERVAL_MIN_MINUTES: i64 = 1;
const INTERVAL_MAX_MINUTES: i64 = 60;

const CLIENT_ID_HELP: &str = "L'ID client del progetto Google Cloud: crea una credenziale OAuth di tipo «App \
    desktop» e copia qui l'ID client. I passaggi sono nella guida «Configurare Gmail».";
const CLIENT_SECRET_HELP: &str = "Il client secret della stessa credenziale OAuth «App desktop». Resta nel \
    portachiavi del sistema e non viene mai mostrato.";
const INTERVAL_HELP: &str = "Ogni quanti minuti controllare la casella (predefinito 2).";
const QUERY_HELP: &str = "Una ricerca nella sintassi di Gmail, per esempio from:mario@gmail.com, \
    subject:fattura o label:lavoro. Si importano solo le email che arrivano dopo la creazione della regola.";
const AUTHORIZE_HELP: &str =
    "Apre il browser sulla pagina di consenso di Google: concedi la lettura della casella e torna all'app.";

pub static DESCRIPTOR: ConnectorDescriptor = ConnectorDescriptor {
    id: ID,
    kind: ConnectorKind::Source,
    label: "Gmail",
    description: "Le email che corrispondono a una ricerca di Gmail diventano thread dell'Agents HUB. Sola lettura: \
        nessuna email viene modificata o cancellata.",
    version: 1,
    config_fields: &[
        ConfigField {
            key: CLIENT_ID_KEY,
            label: "Client ID OAuth",
            help: CLIENT_ID_HELP,
            required: true,
            kind: FieldKind::Text { max_len: CLIENT_ID_MAX, placeholder: "1234567890-abc.apps.googleusercontent.com" },
        },
        ConfigField {
            key: CLIENT_SECRET_SLOT,
            label: "Client secret OAuth",
            help: CLIENT_SECRET_HELP,
            required: true,
            kind: FieldKind::Secret,
        },
        ConfigField {
            key: INTERVAL_KEY,
            label: "Controllo ogni (minuti)",
            help: INTERVAL_HELP,
            required: false,
            kind: FieldKind::Number { min: INTERVAL_MIN_MINUTES, max: INTERVAL_MAX_MINUTES },
        },
    ],
    filter_fields: &[ConfigField {
        key: QUERY_KEY,
        label: "Ricerca",
        help: QUERY_HELP,
        required: true,
        kind: FieldKind::Text { max_len: QUERY_MAX, placeholder: "from:mario@gmail.com" },
    }],
    actions: &[
        Action { id: ActionId::Authorize, label: "Autorizza", help: AUTHORIZE_HELP },
        Action {
            id: ActionId::Test,
            label: "Prova",
            help: "Verifica l'autorizzazione e mostra l'indirizzo dell'account.",
        },
    ],
    permissions: Permissions {
        hosts: &[ACCOUNTS_HOST, OAUTH_HOST, API_HOST],
        hub: &[HubCapability::DeliverMessages, HubCapability::WakeSecretaries],
    },
    limits: DescriptorLimits {
        items_per_round: None,
        min_interval_secs: None,
        default_interval_secs: Some(INTERVAL_DEFAULT_SECS),
    },
    internal_secret_slots: &[REFRESH_TOKEN_SLOT],
};
