//! Da un Bot a cio' che serve per lanciarlo: agente effettivo, modello concreto,
//! effort e skill applicabili. Pura (nessun I/O): l'agente del workspace e il
//! catalogo arrivano dal chiamante. Cio' che l'agente effettivo non sa applicare
//! non blocca il lancio: si toglie e si segnala in `warnings` (in inglese, come
//! quelli di `agents::automation_command`). L'esistenza di una skill utente la
//! verifica chi lancia, perche' serve il disco.

use super::agent::{AgentBot, ModelChoice, SkillRef};
use super::catalog::AgentCatalog;
use crate::agents::presets::PRESETS as PLUGIN_SKILLS;
use crate::agents::{
    effort_is_supported, model_is_valid, tier_models, unsupported_effort_warning, AgentIntegration, AgentUnavailable,
    Effort, ModelTier,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBot {
    pub agent_id: String,
    /// `None` = decide la CLI (nessun `--model`).
    pub model: Option<String>,
    pub effort: Option<Effort>,
    pub skill: Option<SkillRef>,
    pub warnings: Vec<String>,
}

/// Risolve `bot` con l'agente fissato o, se non ne ha, con `workspace_agent`
/// (l'id dell'agente gia' scelto per il workspace). L'agente deve essere nel
/// catalogo e abilitato: altrimenti e' un errore, mai un ripiego su un altro.
pub fn resolve_bot(
    bot: &AgentBot,
    workspace_agent: &str,
    catalog: &AgentCatalog,
) -> Result<ResolvedBot, AgentUnavailable> {
    let agent = catalog.find(bot.agent.as_deref().unwrap_or(workspace_agent))?;
    let mut warnings = Vec::new();
    let model = resolve_model(&bot.model, bot.agent.is_some(), agent, &mut warnings);
    let effort = bot.effort.filter(|&effort| {
        let supported = effort_is_supported(agent, effort);
        if !supported {
            warnings.push(unsupported_effort_warning(agent, effort));
        }
        supported
    });
    let skill = bot.skill.clone().filter(|skill| match skill_obstacle(skill, agent) {
        Some(warning) => {
            warnings.push(warning);
            false
        }
        None => true,
    });
    Ok(ResolvedBot { agent_id: agent.id().to_string(), model, effort, skill, warnings })
}

/// Il modello concreto; `None` (con un avviso se il Bot ne chiedeva uno) quando
/// decide la CLI.
fn resolve_model(
    choice: &ModelChoice,
    agent_fixed: bool,
    agent: &dyn AgentIntegration,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let name = agent.display_name();
    match choice {
        ModelChoice::Default => None,
        ModelChoice::Tier(tier) => {
            let model = tier_models(agent).map(|tiers| match tier {
                ModelTier::Standard => tiers.standard,
                ModelTier::Strong => tiers.strong,
            });
            if model.is_none() {
                warnings
                    .push(format!("{name} does not suggest a {} model: the CLI default is used", tier_label(*tier)));
            }
            model.map(str::to_string)
        }
        ModelChoice::Id(id) if !agent_fixed => {
            warnings.push(format!("the model {id} needs a fixed agent: the {name} default is used"));
            None
        }
        ModelChoice::Id(id) if !model_is_valid(agent, id) => {
            warnings.push(format!("invalid model for {name}: {id}: the CLI default is used"));
            None
        }
        ModelChoice::Id(id) => Some(id.clone()),
    }
}

fn tier_label(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Standard => "standard",
        ModelTier::Strong => "strong",
    }
}

/// Perche' la skill non si puo' dare a quell'agente, se non si puo'.
fn skill_obstacle(skill: &SkillRef, agent: &dyn AgentIntegration) -> Option<String> {
    let name = agent.display_name();
    let caps = agent.capabilities();
    match skill {
        SkillRef::Plugin(id) if !PLUGIN_SKILLS.iter().any(|p| p.id == id) => {
            Some(format!("unknown plugin skill \"{id}\": ignored"))
        }
        SkillRef::Plugin(id) if caps.presets.is_none() => {
            Some(format!("{name} does not ship the plugin skills: skill \"{id}\" ignored"))
        }
        SkillRef::User(id) if caps.user_skills.is_none() => {
            Some(format!("{name} does not support user skills: skill \"{id}\" ignored"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::{FakeIntegration, FAKE_MODEL_LARGE, FAKE_MODEL_SMALL};
    use crate::agents::Capability;
    use crate::bots::presets::{self, GENERAL_ID};

    const FULL: &str = "fake-full";
    const BARE: &str = "fake-bare";
    const OFF: &str = "fake-off";

    fn with_catalog<R>(f: impl FnOnce(&AgentCatalog) -> R) -> R {
        let full = FakeIntegration::full(FULL);
        let bare = FakeIntegration::bare(BARE, "fake-cli");
        let off = FakeIntegration::full(OFF);
        let registered = [&full as &dyn AgentIntegration, &bare, &off];
        f(&AgentCatalog::new(&registered, vec![FULL.into(), BARE.into()]))
    }

    fn resolve(bot: &AgentBot, workspace: &str) -> Result<ResolvedBot, AgentUnavailable> {
        with_catalog(|c| resolve_bot(bot, workspace, c))
    }

    fn preset(id: &str) -> AgentBot {
        presets::find(id).unwrap().bot()
    }

    #[test]
    fn the_general_bot_follows_the_workspace_with_the_cli_defaults() {
        let resolved = resolve(&preset(GENERAL_ID), FULL).unwrap();
        assert_eq!(
            resolved,
            ResolvedBot { agent_id: FULL.into(), model: None, effort: None, skill: None, warnings: vec![] }
        );
    }

    #[test]
    fn a_tier_resolves_on_the_effective_agent() {
        let intake = resolve(&preset("intake"), FULL).unwrap();
        assert_eq!(intake.model.as_deref(), Some(FAKE_MODEL_LARGE));
        let setup = resolve(&preset("setup"), FULL).unwrap();
        assert_eq!(setup.model.as_deref(), Some(FAKE_MODEL_SMALL));
    }

    #[test]
    fn a_tier_without_suggestions_falls_back_to_the_cli_default_with_a_warning() {
        let bot = AgentBot { effort: None, skill: None, ..preset("intake") };
        let resolved = resolve(&bot, BARE).unwrap();
        assert_eq!(resolved.model, None);
        assert_eq!(resolved.warnings, ["Fake does not suggest a strong model: the CLI default is used"]);
    }

    #[test]
    fn a_fixed_agent_wins_over_the_workspace() {
        let bot = AgentBot { agent: Some(FULL.into()), ..preset("intake") };
        assert_eq!(resolve(&bot, BARE).unwrap().agent_id, FULL);
        assert_eq!(resolve(&preset("intake"), BARE).unwrap().agent_id, BARE);
    }

    #[test]
    fn an_unusable_agent_is_an_error() {
        let bot = AgentBot { agent: Some(OFF.into()), ..preset(GENERAL_ID) };
        assert!(matches!(resolve(&bot, FULL), Err(AgentUnavailable::Disabled { .. })));
        assert_eq!(resolve(&preset(GENERAL_ID), "gone").err(), Some(AgentUnavailable::Unknown("gone".into())));
    }

    #[test]
    fn a_model_id_is_used_only_with_the_fixed_agent_that_knows_it() {
        let id = |model: &str, agent: Option<&str>| AgentBot {
            model: ModelChoice::Id(model.into()),
            agent: agent.map(str::to_string),
            ..preset(GENERAL_ID)
        };
        let ok = resolve(&id(FAKE_MODEL_SMALL, Some(FULL)), BARE).unwrap();
        assert_eq!((ok.model.as_deref(), ok.warnings.len()), (Some(FAKE_MODEL_SMALL), 0));
        let unfixed = resolve(&id(FAKE_MODEL_SMALL, None), FULL).unwrap();
        assert_eq!(unfixed.model, None);
        assert_eq!(unfixed.warnings, ["the model fake-small needs a fixed agent: the Fake default is used"]);
        let invalid = resolve(&id("other-model", Some(FULL)), FULL).unwrap();
        assert_eq!(invalid.model, None);
        assert_eq!(invalid.warnings, ["invalid model for Fake: other-model: the CLI default is used"]);
    }

    #[test]
    fn an_unsupported_effort_is_dropped_with_a_warning() {
        let high = resolve(&AgentBot { skill: None, ..preset("intake") }, FULL).unwrap();
        assert_eq!((high.effort, high.warnings.len()), (Some(Effort::High), 0));
        let medium = resolve(&AgentBot { skill: None, ..preset("setup") }, FULL).unwrap();
        assert_eq!(medium.effort, None);
        assert_eq!(medium.warnings, ["Fake does not support the effort level \"medium\": ignored"]);
    }

    #[test]
    fn plugin_and_user_skills_pass_when_the_agent_supports_them() {
        let intake = resolve(&preset("intake"), FULL).unwrap();
        assert_eq!(intake.skill, Some(SkillRef::Plugin("task-intake".into())));
        let user = AgentBot { skill: Some(SkillRef::User("my-skill".into())), ..preset(GENERAL_ID) };
        assert_eq!(resolve(&user, FULL).unwrap().skill, Some(SkillRef::User("my-skill".into())));
    }

    #[test]
    fn skills_the_agent_cannot_take_are_dropped_with_a_warning() {
        let plugin = AgentBot { skill: Some(SkillRef::Plugin("task-intake".into())), ..preset(GENERAL_ID) };
        let no_plugin = resolve(&plugin, BARE).unwrap();
        assert_eq!(no_plugin.skill, None);
        assert_eq!(no_plugin.warnings, ["Fake does not ship the plugin skills: skill \"task-intake\" ignored"]);

        let user = AgentBot { skill: Some(SkillRef::User("my-skill".into())), ..preset(GENERAL_ID) };
        let no_user = resolve(&user, BARE).unwrap();
        assert_eq!(no_user.skill, None);
        assert_eq!(no_user.warnings, ["Fake does not support user skills: skill \"my-skill\" ignored"]);

        let retired = AgentBot { skill: Some(SkillRef::Plugin("retired".into())), ..preset(GENERAL_ID) };
        assert_eq!(resolve(&retired, FULL).unwrap().warnings, ["unknown plugin skill \"retired\": ignored"]);
    }

    #[test]
    fn warnings_accumulate_in_order() {
        let bare_without = FakeIntegration::full(FULL).without(Capability::Models).without(Capability::Effort);
        let registered = [&bare_without as &dyn AgentIntegration];
        let catalog = AgentCatalog::new(&registered, vec![FULL.into()]);
        let resolved = resolve_bot(&preset("intake"), FULL, &catalog).unwrap();
        assert_eq!(resolved.warnings.len(), 2);
        assert!(resolved.warnings[0].contains("strong model"));
        assert!(resolved.warnings[1].contains("effort"));
        assert_eq!(resolved.skill, Some(SkillRef::Plugin("task-intake".into())));
    }
}
