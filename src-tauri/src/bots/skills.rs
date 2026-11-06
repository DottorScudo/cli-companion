//! Le skill che si possono dare a un Bot, per l'interfaccia: quelle del plugin
//! e quelle globali dell'utente. Puro: le skill utente arrivano gia' lette.

use super::agent::SkillRef;
use crate::agents::presets::PRESETS as PLUGIN_SKILLS;
use crate::user_skills::SkillInfo;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BotSkillOption {
    pub skill: SkillRef,
    /// Nome mostrato.
    pub name: String,
    pub description: String,
}

/// Prima le skill del plugin (nell'ordine del registro), poi quelle dell'utente.
pub fn selectable_skills(user: Vec<SkillInfo>) -> Vec<BotSkillOption> {
    let plugin = PLUGIN_SKILLS.iter().map(|p| BotSkillOption {
        skill: SkillRef::Plugin(p.id.to_string()),
        name: p.name.to_string(),
        description: p.description(),
    });
    let user = user.into_iter().map(|s| BotSkillOption {
        skill: SkillRef::User(s.name.clone()),
        name: s.name,
        description: s.description,
    });
    plugin.chain(user).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_skills_come_first_then_the_user_ones() {
        let user = vec![SkillInfo { name: "my-skill".into(), description: "Does things.".into() }];
        let options = selectable_skills(user);
        assert_eq!(options.len(), PLUGIN_SKILLS.len() + 1);
        assert_eq!(options[0].skill, SkillRef::Plugin(PLUGIN_SKILLS[0].id.into()));
        assert_eq!(options[0].name, PLUGIN_SKILLS[0].name);
        assert!(options[..PLUGIN_SKILLS.len()].iter().all(|o| !o.description.is_empty()));
        let last = options.last().unwrap();
        assert_eq!(
            last,
            &BotSkillOption {
                skill: SkillRef::User("my-skill".into()),
                name: "my-skill".into(),
                description: "Does things.".into()
            }
        );
    }

    #[test]
    fn without_user_skills_only_the_plugin_ones_remain() {
        assert_eq!(selectable_skills(Vec::new()).len(), PLUGIN_SKILLS.len());
    }
}
