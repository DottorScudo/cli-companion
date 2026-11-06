//! La skill di un worker: il nome con cui il suo agente la carica, ricavato dai
//! contratti `PluginSkills` e `UserSkills`. Il worker lo legge in `get_task`.

use super::{is_safe_launch_token, AgentIntegration, LaunchSkill};

/// Lunghezza massima del nome con cui un agente carica una skill.
const SKILL_NAME_MAX: usize = 100;

/// Il nome che un agente da' a una skill e' semplice: lettere, cifre e `-_.:`
/// (il `:` separa il plugin, come `plugin:skill`). Lo legge il modello in `get_task`.
pub(super) fn is_plain_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= SKILL_NAME_MAX
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// Il nome con cui l'agente carica `skill` col proprio strumento delle skill.
/// `None` se l'agente non dichiara il contratto della skill, se il nome della
/// skill non e' sicuro per una riga di comando o se il nome che l'agente ne
/// ricava non e' semplice.
pub fn worker_skill_invocation(integration: &dyn AgentIntegration, skill: LaunchSkill) -> Option<String> {
    let caps = integration.capabilities();
    let name = match skill {
        LaunchSkill::Plugin(id) if is_safe_launch_token(id) => caps.plugin_skills?.skill_name(id),
        LaunchSkill::User(name) if is_safe_launch_token(name) => caps.user_skills?.skill_name(name),
        _ => return None,
    };
    is_plain_skill_name(&name).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;
    use crate::agents::Capability;

    #[test]
    fn each_kind_of_skill_gets_the_name_of_its_contract() {
        let agent = FakeIntegration::full("fake");
        assert_eq!(
            worker_skill_invocation(&agent, LaunchSkill::Plugin("task-intake")).as_deref(),
            Some("fake-plugin:task-intake")
        );
        assert_eq!(worker_skill_invocation(&agent, LaunchSkill::User("my-skill")).as_deref(), Some("my-skill"));
    }

    #[test]
    fn an_agent_without_the_contract_has_no_invocation() {
        let no_plugin = FakeIntegration::full("fake").without(Capability::PluginSkills);
        assert_eq!(worker_skill_invocation(&no_plugin, LaunchSkill::Plugin("task-intake")), None);
        assert!(worker_skill_invocation(&no_plugin, LaunchSkill::User("my-skill")).is_some());
        let no_user = FakeIntegration::full("fake").without(Capability::UserSkills);
        assert_eq!(worker_skill_invocation(&no_user, LaunchSkill::User("my-skill")), None);
    }

    #[test]
    fn a_plain_skill_name_has_no_spaces_prefixes_or_quotes() {
        for good in ["task-intake", "cli-companion-notify:task-intake", "my_skill.v2"] {
            assert!(is_plain_skill_name(good), "{good:?}");
        }
        for bad in ["", "/task-intake", "a b", "a\"b", "a$b", &"a".repeat(SKILL_NAME_MAX + 1)] {
            assert!(!is_plain_skill_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_name_that_is_not_safe_for_a_command_line_has_no_invocation() {
        let agent = FakeIntegration::full("fake");
        for bad in ["", "a b", "a$b", "a\"b"] {
            assert_eq!(worker_skill_invocation(&agent, LaunchSkill::Plugin(bad)), None, "{bad:?}");
            assert_eq!(worker_skill_invocation(&agent, LaunchSkill::User(bad)), None, "{bad:?}");
        }
    }
}
