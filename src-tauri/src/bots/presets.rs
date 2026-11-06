//! I Bot predefiniti: tutti partono con l'agente del workspace. Aggiungerne uno
//! = una voce in `PRESETS` + una migrazione additiva di `bots.json` che lo crea
//! (vedi `migrations`): i file esistenti lo ricevono una volta sola e un Bot
//! cancellato non ricompare. L'id e' stabile: lo ricorda il Bot creato
//! (`AgentBot::preset`) e permette di ripristinarlo. Gli id dei Bot creati
//! dall'utente cominciano con `USER_ID_PREFIX`, che nessun preset usa.

use super::agent::{AgentBot, ModelChoice, SkillRef};
use crate::agents::{Effort, ModelTier};
use crate::board::TaskKind;

/// Il Bot generico, obbligatorio: configurabile ma non cancellabile ne' disattivabile.
pub const GENERAL_ID: &str = "general";

/// Prefisso degli id dei Bot creati dall'utente.
pub const USER_ID_PREFIX: &str = "custom-";

/// Un Bot predefinito.
pub struct BotPreset {
    pub id: &'static str,
    pub name: &'static str,
    /// Il compito, in inglese: lo legge il modello.
    pub task: &'static str,
    pub model: ModelChoice,
    pub effort: Option<Effort>,
    /// Id di una skill del plugin (`agents::presets::PRESETS`).
    pub skill: Option<&'static str>,
    /// I tipi di task automatici che esegue; vuoto = solo interattivo.
    pub kinds: &'static [TaskKind],
}

impl BotPreset {
    /// Il Bot appena creato da questo preset: acceso, senza immagine.
    pub fn bot(&self) -> AgentBot {
        AgentBot {
            id: self.id.to_string(),
            name: self.name.to_string(),
            task: self.task.to_string(),
            agent: None,
            model: self.model.clone(),
            effort: self.effort,
            skill: self.skill(),
            kinds: self.kinds.to_vec(),
            enabled: true,
            image: None,
            preset: Some(self.id.to_string()),
        }
    }

    pub fn skill(&self) -> Option<SkillRef> {
        self.skill.map(|id| SkillRef::Plugin(id.to_string()))
    }
}

/// I preset, nell'ordine in cui compaiono.
pub static PRESETS: [BotPreset; 6] = [
    BotPreset {
        id: GENERAL_ID,
        name: "General",
        task: "General-purpose coding agent.",
        model: ModelChoice::Default,
        effort: None,
        skill: None,
        kinds: &[TaskKind::Work],
    },
    BotPreset {
        id: "intake",
        name: "Intake",
        task: "Turns requests into queue tasks.",
        model: ModelChoice::Tier(ModelTier::Strong),
        effort: Some(Effort::High),
        skill: Some("task-intake"),
        kinds: &[],
    },
    BotPreset {
        id: "setup",
        name: "Setup",
        task: "Configures the workspace and the worker rules.",
        model: ModelChoice::Tier(ModelTier::Standard),
        effort: Some(Effort::Medium),
        skill: Some("project-setup"),
        kinds: &[],
    },
    BotPreset {
        id: "skill-wizard",
        name: "Skill Wizard",
        task: "Creates and manages global skills.",
        model: ModelChoice::Tier(ModelTier::Standard),
        effort: Some(Effort::Medium),
        skill: Some("skill-wizard"),
        kinds: &[],
    },
    BotPreset {
        id: "reviewer",
        name: "Reviewer",
        task: "Verifies the integrated result of finished tasks and proposes fixes.",
        model: ModelChoice::Tier(ModelTier::Strong),
        effort: Some(Effort::High),
        skill: None,
        kinds: &[TaskKind::Review],
    },
    BotPreset {
        id: "planner",
        name: "Planner",
        task: "Explores the repository read-only and proposes queue tasks.",
        model: ModelChoice::Tier(ModelTier::Strong),
        effort: Some(Effort::High),
        skill: None,
        kinds: &[TaskKind::Plan],
    },
];

/// Il preset con quell'id.
pub fn find(id: &str) -> Option<&'static BotPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Tutti i Bot predefiniti, come in un file nuovo.
pub fn default_bots() -> Vec<AgentBot> {
    PRESETS.iter().map(BotPreset::bot).collect()
}

/// Il Bot `general` e' sempre presente e acceso: se manca (file scritto a mano)
/// torna in testa all'elenco.
pub fn ensure_general(bots: &mut Vec<AgentBot>) {
    match bots.iter_mut().find(|b| b.id == GENERAL_ID) {
        Some(general) => general.enabled = true,
        None => bots.insert(0, PRESETS[0].bot()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::presets::PRESETS as PLUGIN_SKILLS;
    use std::collections::HashSet;

    #[test]
    fn the_presets_come_in_order() {
        let ids: Vec<_> = PRESETS.iter().map(|p| (p.id, p.name)).collect();
        assert_eq!(
            ids,
            [
                ("general", "General"),
                ("intake", "Intake"),
                ("setup", "Setup"),
                ("skill-wizard", "Skill Wizard"),
                ("reviewer", "Reviewer"),
                ("planner", "Planner"),
            ]
        );
        assert_eq!(PRESETS[0].id, GENERAL_ID);
    }

    #[test]
    fn each_preset_has_the_model_effort_and_skill_of_its_job() {
        let fields = |id| {
            let p = find(id).unwrap();
            (p.model.clone(), p.effort, p.skill)
        };
        assert_eq!(fields("general"), (ModelChoice::Default, None, None));
        assert_eq!(fields("intake"), (ModelChoice::Tier(ModelTier::Strong), Some(Effort::High), Some("task-intake")));
        assert_eq!(
            fields("setup"),
            (ModelChoice::Tier(ModelTier::Standard), Some(Effort::Medium), Some("project-setup"))
        );
        assert_eq!(
            fields("skill-wizard"),
            (ModelChoice::Tier(ModelTier::Standard), Some(Effort::Medium), Some("skill-wizard"))
        );
        for id in ["reviewer", "planner"] {
            assert_eq!(fields(id), (ModelChoice::Tier(ModelTier::Strong), Some(Effort::High), None), "{id}");
        }
        assert!(find("nope").is_none());
    }

    #[test]
    fn each_preset_runs_only_the_task_kinds_of_its_job() {
        let kinds = |id| find(id).unwrap().kinds;
        assert_eq!(kinds("general"), [TaskKind::Work]);
        assert_eq!(kinds("reviewer"), [TaskKind::Review]);
        assert_eq!(kinds("planner"), [TaskKind::Plan]);
        // Le skill dei preset sono pensate per un terminale interattivo.
        for id in ["intake", "setup", "skill-wizard"] {
            assert!(kinds(id).is_empty(), "{id}");
        }
        assert_eq!(find("reviewer").unwrap().bot().kinds, [TaskKind::Review]);
    }

    #[test]
    fn presets_are_well_formed() {
        let ids: HashSet<_> = PRESETS.iter().map(|p| p.id).collect();
        let names: HashSet<_> = PRESETS.iter().map(|p| p.name.to_lowercase()).collect();
        assert_eq!((ids.len(), names.len()), (PRESETS.len(), PRESETS.len()));
        for p in &PRESETS {
            assert!(!p.id.starts_with(USER_ID_PREFIX), "{}", p.id);
            assert_eq!(super::super::validate::normalize_name(p.name).unwrap(), p.name);
            assert!(p.task.is_ascii() && !p.task.is_empty(), "{}", p.id);
            // Nessun preset fissa un modello concreto: deve valere con ogni agente.
            assert!(!matches!(p.model, ModelChoice::Id(_)), "{}", p.id);
            if let Some(skill) = p.skill {
                assert!(PLUGIN_SKILLS.iter().any(|s| s.id == skill), "{skill}");
            }
            assert_eq!(super::super::validate::normalize_kinds(p.kinds.to_vec()), p.kinds, "{}", p.id);
        }
    }

    #[test]
    fn a_preset_bot_follows_the_workspace_and_remembers_its_preset() {
        let bot = find("intake").unwrap().bot();
        assert_eq!(bot.agent, None);
        assert!(bot.enabled && bot.image.is_none());
        assert_eq!(bot.preset.as_deref(), Some("intake"));
        assert_eq!(bot.skill, Some(SkillRef::Plugin("task-intake".into())));
    }

    #[test]
    fn general_is_put_back_and_kept_enabled() {
        let mut empty = Vec::new();
        ensure_general(&mut empty);
        assert_eq!(empty, vec![PRESETS[0].bot()]);

        let mut others = vec![find("setup").unwrap().bot()];
        ensure_general(&mut others);
        assert_eq!(others.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["general", "setup"]);

        let mut off = default_bots();
        off[0].enabled = false;
        off[0].name = "Generic".into();
        ensure_general(&mut off);
        assert!(off[0].enabled);
        assert_eq!(off[0].name, "Generic");
        assert_eq!(off.len(), PRESETS.len());
    }
}
