//! Il Bot come agente specializzato: con quale coding agent parte, con quale
//! modello, effort e skill. Solo i tipi persistiti in `bots.json` e scambiati
//! con il frontend; validazione in `validate`, risoluzione in `resolve`.

use super::BotImage;
use crate::agents::{Effort, LaunchSkill, ModelTier};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Il modello di un Bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[ts(export)]
pub enum ModelChoice {
    /// Decide la CLI: nessun modello nella riga di comando.
    Default,
    /// Il modello consigliato per la fascia dall'agente effettivo: vale con
    /// qualunque agente.
    Tier(ModelTier),
    /// Un modello concreto: ammesso solo con un agente fissato.
    Id(String),
}

/// La skill di un Bot: vale sia nei tab interattivi sia nei worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
#[ts(export)]
pub enum SkillRef {
    /// Una skill del plugin (id di `agents::presets::PRESETS`).
    Plugin(String),
    /// Una skill globale dell'utente (nome in `user_skills`): la sua esistenza
    /// si verifica al lancio, perche' le skill si sincronizzano in background.
    User(String),
}

impl SkillRef {
    /// La skill come la chiedono i contratti di avvio degli agenti.
    pub fn launch_skill(&self) -> LaunchSkill<'_> {
        match self {
            SkillRef::Plugin(id) => LaunchSkill::Plugin(id),
            SkillRef::User(name) => LaunchSkill::User(name),
        }
    }

    /// L'id della skill del plugin o il nome di quella dell'utente.
    pub fn id(&self) -> &str {
        match self {
            SkillRef::Plugin(id) | SkillRef::User(id) => id,
        }
    }
}

/// Un Bot-agente, globale (non per workspace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentBot {
    /// Slug stabile e unico.
    pub id: String,
    /// Nome mostrato, unico senza distinguere le maiuscole.
    pub name: String,
    /// Il compito, in inglese: lo legge il modello che sceglie il Bot.
    pub task: String,
    /// `None` = l'agente del workspace.
    pub agent: Option<String>,
    pub model: ModelChoice,
    pub effort: Option<Effort>,
    pub skill: Option<SkillRef>,
    pub enabled: bool,
    pub image: Option<BotImage>,
    /// Il Bot predefinito da cui nasce: permette di ripristinarlo.
    pub preset: Option<String>,
}

/// I campi di un Bot che l'utente sceglie, alla creazione e alla modifica.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
pub struct AgentBotDraft {
    pub name: String,
    pub task: String,
    pub agent: Option<String>,
    pub model: ModelChoice,
    pub effort: Option<Effort>,
    pub skill: Option<SkillRef>,
    pub enabled: bool,
    /// L'immagine voluta (`None` la toglie); il backend la convalida.
    pub image: Option<BotImage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_choices_have_a_stable_json_shape() {
        let shapes = [
            (ModelChoice::Default, json!({"kind": "default"})),
            (ModelChoice::Tier(ModelTier::Strong), json!({"kind": "tier", "value": "strong"})),
            (ModelChoice::Id("m-1".into()), json!({"kind": "id", "value": "m-1"})),
        ];
        for (choice, shape) in shapes {
            assert_eq!(serde_json::to_value(&choice).unwrap(), shape);
            assert_eq!(serde_json::from_value::<ModelChoice>(shape).unwrap(), choice);
        }
        assert!(serde_json::from_value::<ModelChoice>(json!({"kind": "tier", "value": "huge"})).is_err());
    }

    #[test]
    fn skill_refs_have_a_stable_json_shape() {
        let plugin = SkillRef::Plugin("task-intake".into());
        assert_eq!(serde_json::to_value(&plugin).unwrap(), json!({"kind": "plugin", "id": "task-intake"}));
        let user: SkillRef = serde_json::from_value(json!({"kind": "user", "id": "my-skill"})).unwrap();
        assert_eq!(user, SkillRef::User("my-skill".into()));
    }
}
