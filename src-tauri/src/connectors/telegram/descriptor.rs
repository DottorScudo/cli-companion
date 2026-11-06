//! Il descrittore di Telegram e il filtro delle notifiche per tipo (pura).

use crate::connectors::descriptor::{
    Action, ActionId, ConfigField, ConnectorDescriptor, ConnectorKind, DescriptorLimits, FieldKind, HubCapability,
    Permissions,
};
use crate::hub::NoticeCategory;
use serde_json::Value;

/// L'id del connettore.
pub const ID: &str = "telegram";
/// La chiave del campo segreto col token del bot.
pub const TOKEN_SLOT: &str = "bot_token";
/// L'unico host della Bot API.
pub const API_HOST: &str = "api.telegram.org";

const TOKEN_HELP: &str = "Apri una chat con @BotFather su Telegram, scrivi /newbot e scegli un nome e uno username \
    che finisca con «bot»: BotFather risponde con il token. Incollalo qui: resta nel portachiavi del sistema e non \
    viene mai mostrato.";

const fn switch(key: &'static str, label: &'static str, help: &'static str, default: bool) -> ConfigField {
    ConfigField { key, label, help, required: false, kind: FieldKind::Bool { default } }
}

pub static DESCRIPTOR: ConnectorDescriptor = ConnectorDescriptor {
    id: ID,
    kind: ConnectorKind::Contact,
    label: "Telegram",
    description: "Ricevi su Telegram le notifiche dell'Agents HUB e scrivi ai segretari: in una chat privata con il \
        tuo bot, oppure in un gruppo con i Topic attivi, dove ogni thread ha il suo topic «canale › titolo».",
    version: 1,
    config_fields: &[
        ConfigField {
            key: TOKEN_SLOT,
            label: "Token del bot",
            help: TOKEN_HELP,
            required: true,
            kind: FieldKind::Secret,
        },
        switch("notify_replies", "Risposte dei segretari", "Notifica quando un segretario risponde.", true),
        switch("notify_questions", "Domande dei worker", "Notifica quando un worker pone una domanda.", true),
        switch("notify_failures", "Task falliti", "Notifica quando un task fallisce.", true),
        switch("notify_attention", "Task che richiedono attenzione", "Notifica quando un task ha bisogno di te.", true),
        switch("notify_proposals", "Nuove proposte", "Notifica quando nasce una nuova proposta.", true),
        switch(
            "notify_plans",
            "Pianificazioni senza proposta",
            "Notifica una pianificazione conclusa senza proposta.",
            true,
        ),
        switch("notify_conclusions", "Thread conclusi", "Notifica quando un thread si conclude.", true),
        switch("notify_done", "Task e revisioni finiti", "Notifica quando un task o una revisione finiscono.", false),
    ],
    filter_fields: &[],
    actions: &[
        Action { id: ActionId::Test, label: "Prova", help: "Verifica il token e mostra lo username del bot." },
        Action {
            id: ActionId::Pair,
            label: "Abbina un account",
            help: "Genera un codice. Chat privata: apri il link da Telegram (o invia /start con il codice al bot). \
                Gruppo con i Topic: aggiungi il bot come amministratore con il permesso «Gestire i topic» e scrivi \
                /start con il codice nel gruppo.",
        },
    ],
    permissions: Permissions { hosts: &[API_HOST], hub: &[HubCapability::ReceiveNotices, HubCapability::SendAsUser] },
    limits: DescriptorLimits::NONE,
    internal_secret_slots: &[],
};

/// Il campo-interruttore che governa un tipo di notifica (`None` = mai inoltrata).
fn notice_key(category: NoticeCategory) -> Option<&'static str> {
    match category {
        NoticeCategory::Reply => Some("notify_replies"),
        NoticeCategory::Question => Some("notify_questions"),
        NoticeCategory::TaskFailed => Some("notify_failures"),
        NoticeCategory::TaskAttention => Some("notify_attention"),
        NoticeCategory::ProposalCreated => Some("notify_proposals"),
        NoticeCategory::PlanWithoutProposal => Some("notify_plans"),
        NoticeCategory::ThreadConcluded => Some("notify_conclusions"),
        NoticeCategory::ReviewDone | NoticeCategory::TaskDone => Some("notify_done"),
        // Gli elementi esterni non si inoltrano (v1).
        NoticeCategory::External => None,
    }
}

/// Il valore scelto dall'utente o, se manca, il predefinito del descrittore.
fn switch_value(config: &Value, key: &str) -> bool {
    let default = match DESCRIPTOR.config_field(key).map(|f| f.kind) {
        Some(FieldKind::Bool { default }) => default,
        _ => false,
    };
    config.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// La configurazione chiede di inoltrare le notifiche di questo tipo?
pub fn notice_wanted(config: &Value, category: NoticeCategory) -> bool {
    notice_key(category).is_some_and(|key| switch_value(config, key))
}

#[cfg(test)]
mod tests;
