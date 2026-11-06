//! I Bot derivati: la copia di un Bot predefinito con un agente fissato. Nascono
//! dalle vecchie voci di lancio "preset con l'agente scelto" (vedi
//! `launch_migration`), cosi' la scelta dell'agente non va persa.

use super::agent::{AgentBot, ModelChoice};
use super::agent_bots::free_name;
use super::presets;
use super::validate::NAME_MAX_CHARS;

/// Il Bot `id` copiato dal predefinito `base` con l'agente `agent`: compito,
/// modello, effort, skill e tipi di task del Bot `base` dell'elenco, o del preset se l'utente
/// lo ha cancellato. Il nome dice l'agente (`Intake (Nome agente)`) ed e' libero
/// fra i Bot e `taken`. `None` se `base` non e' un Bot predefinito.
pub fn derived_bot(
    list: &[AgentBot],
    id: String,
    base: &str,
    agent: &str,
    agent_name: &str,
    taken: &[String],
) -> Option<AgentBot> {
    let preset = presets::find(base)?;
    let source = list.iter().find(|b| b.id == base).cloned().unwrap_or_else(|| preset.bot());
    // Un modello concreto vale solo con l'agente per cui e' stato scelto.
    let model = match source.model {
        ModelChoice::Id(_) if source.agent.as_deref() != Some(agent) => preset.model.clone(),
        model => model,
    };
    Some(AgentBot {
        id,
        name: free_name(list, &derived_name(&source.name, agent_name), taken),
        task: source.task,
        agent: Some(agent.to_string()),
        model,
        effort: source.effort,
        skill: source.skill,
        kinds: source.kinds,
        enabled: true,
        image: None,
        preset: Some(preset.id.to_string()),
    })
}

/// `<base> (<agente>)` entro la lunghezza massima dei nomi: si accorcia il
/// nome del Bot, poi (se non basta) l'insieme.
fn derived_name(base: &str, agent_name: &str) -> String {
    let full = format!("{base} ({agent_name})");
    let suffix_chars = agent_name.chars().count() + " ()".len();
    if full.chars().count() <= NAME_MAX_CHARS {
        return full;
    }
    if suffix_chars >= NAME_MAX_CHARS {
        return full.chars().take(NAME_MAX_CHARS).collect::<String>().trim_end().to_string();
    }
    let room = NAME_MAX_CHARS - suffix_chars;
    let short: String = base.chars().take(room).collect();
    format!("{} ({agent_name})", short.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Effort, ModelTier};
    use crate::bots::agent::SkillRef;
    use crate::bots::presets::default_bots;

    fn derive(list: &[AgentBot], base: &str, agent_name: &str, taken: &[String]) -> AgentBot {
        derived_bot(list, format!("{base}-x"), base, "x", agent_name, taken).unwrap()
    }

    #[test]
    fn it_copies_the_job_of_the_preset_bot_with_the_agent_fixed() {
        let bot = derive(&default_bots(), "intake", "Other", &[]);
        assert_eq!(bot.id, "intake-x");
        assert_eq!(bot.name, "Intake (Other)");
        assert_eq!(bot.agent.as_deref(), Some("x"));
        assert_eq!(bot.model, ModelChoice::Tier(ModelTier::Strong));
        assert_eq!((bot.effort, bot.skill), (Some(Effort::High), Some(SkillRef::Plugin("task-intake".into()))));
        assert_eq!(bot.task, presets::find("intake").unwrap().task);
        assert!(bot.enabled && bot.image.is_none());
        assert_eq!(bot.preset.as_deref(), Some("intake"));
    }

    #[test]
    fn the_user_changes_to_the_preset_bot_are_kept() {
        let mut list = default_bots();
        let setup = list.iter_mut().find(|b| b.id == "setup").unwrap();
        setup.name = "Config".into();
        setup.task = "My task.".into();
        setup.effort = Some(Effort::Low);
        let bot = derive(&list, "setup", "Other", &[]);
        assert_eq!(
            (bot.name.as_str(), bot.task.as_str(), bot.effort),
            ("Config (Other)", "My task.", Some(Effort::Low))
        );
    }

    #[test]
    fn a_deleted_preset_bot_is_copied_from_the_preset() {
        let list: Vec<_> = default_bots().into_iter().filter(|b| b.id != "skill-wizard").collect();
        let bot = derive(&list, "skill-wizard", "Ag", &[]);
        assert_eq!(bot.name, "Skill Wizard (Ag)");
        assert_eq!(bot.skill, Some(SkillRef::Plugin("skill-wizard".into())));
    }

    #[test]
    fn a_concrete_model_of_another_agent_goes_back_to_the_preset_tier() {
        let mut list = default_bots();
        let intake = list.iter_mut().find(|b| b.id == "intake").unwrap();
        intake.agent = Some("y".into());
        intake.model = ModelChoice::Id("y-model".into());
        assert_eq!(derive(&list, "intake", "Other", &[]).model, ModelChoice::Tier(ModelTier::Strong));
        let same = derived_bot(&list, "intake-y".into(), "intake", "y", "Y", &[]).unwrap();
        assert_eq!(same.model, ModelChoice::Id("y-model".into()));
    }

    #[test]
    fn the_name_fits_and_is_free() {
        let bot = derive(&default_bots(), "skill-wizard", "Some Long Agent", &[]);
        assert_eq!(bot.name, "Skill (Some Long Agent)");
        let huge = derive(&default_bots(), "intake", &"A".repeat(40), &[]);
        assert_eq!(huge.name.chars().count(), NAME_MAX_CHARS);
        let taken = derive(&default_bots(), "intake", "Other", &["intake (other)".to_string()]);
        assert_eq!(taken.name, "Intake (Other) 2");
    }

    #[test]
    fn only_preset_bots_can_be_derived() {
        assert!(derived_bot(&default_bots(), "custom-a-x".into(), "custom-a", "x", "X", &[]).is_none());
    }
}
