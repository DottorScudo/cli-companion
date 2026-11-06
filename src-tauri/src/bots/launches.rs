//! I Bot nel menu di avvio e il lancio di un Bot in un tab. Puro (nessun I/O):
//! l'agente del workspace, il catalogo e l'esistenza delle skill utente arrivano
//! dal chiamante. Il comando lo costruisce il contratto `InteractiveLauncher`
//! dell'agente effettivo; qui non c'e' nulla di proprio di un agente.

use super::agent::{AgentBot, SkillRef};
use super::catalog::AgentCatalog;
use super::resolve::{resolve_bot, ResolvedBot};
use crate::agents::{
    first_invalid_env_name, AgentIntegration, AgentUnavailable, InteractiveLauncher, InteractiveRequest, LaunchSkill,
};
use serde::Serialize;
use ts_rs::TS;

/// Prefisso degli id delle voci del menu di avvio che aprono un Bot.
pub const BOT_LAUNCH_PREFIX: &str = "bot:";

/// La voce di lancio del Bot `bot_id`: `bot:<id>`.
pub fn bot_launch_id(bot_id: &str) -> String {
    format!("{BOT_LAUNCH_PREFIX}{bot_id}")
}

/// Le voci dei Bot abilitati: i valori che un avvio predefinito puo' prendere
/// oltre a quelli integrati e del workspace.
pub fn enabled_launch_ids(bots: &[AgentBot]) -> Vec<String> {
    bots.iter().filter(|b| b.enabled).map(|b| bot_launch_id(&b.id)).collect()
}

/// Voce del menu di avvio per un Bot abilitato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BotLaunch {
    /// `bot:<id del Bot>`.
    pub id: String,
    pub bot_id: String,
    pub name: String,
    /// L'agente effettivo; `None` se non si e' potuto risolvere.
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    /// Perche' non si puo' avviare (in italiano); `None` = avviabile.
    pub blocked_reason: Option<String>,
}

/// Lancio pronto di un Bot in un tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BotTabLaunch {
    pub name: String,
    pub agent_id: String,
    pub command: String,
    /// Salvato nella sessione per il ripristino: senza la skill.
    pub restore_command: String,
    /// Cio' che l'agente non sa applicare (in inglese, da `resolve_bot`).
    pub warnings: Vec<String>,
}

/// Perche' l'agente non si puo' usare, per l'utente.
fn unavailable_message(error: &AgentUnavailable) -> String {
    match error {
        AgentUnavailable::Unknown(id) => format!("L'agente «{id}» del Bot non è registrato."),
        AgentUnavailable::Disabled { name, .. } => format!("{name} è disabilitato: abilitalo nelle Integrazioni."),
        AgentUnavailable::MissingContract { name, .. } => unsupported_message(name),
    }
}

fn unsupported_message(agent_name: &str) -> String {
    format!("{agent_name} non supporta l'avvio dei Bot.")
}

/// Il Bot risolto insieme all'integrazione effettiva.
fn resolve<'a>(
    bot: &AgentBot,
    workspace_agent: &str,
    catalog: &AgentCatalog<'a>,
) -> Result<(&'a dyn AgentIntegration, ResolvedBot), AgentUnavailable> {
    let resolved = resolve_bot(bot, workspace_agent, catalog)?;
    Ok((catalog.find(&resolved.agent_id)?, resolved))
}

/// Una voce per ogni Bot abilitato, nell'ordine dell'elenco. La prontezza
/// dell'agente nell'ambiente la valuta il frontend dall'`agent_id`.
pub fn bot_launches(bots: &[AgentBot], workspace_agent: &str, catalog: &AgentCatalog) -> Vec<BotLaunch> {
    bots.iter().filter(|bot| bot.enabled).map(|bot| bot_launch(bot, workspace_agent, catalog)).collect()
}

fn bot_launch(bot: &AgentBot, workspace_agent: &str, catalog: &AgentCatalog) -> BotLaunch {
    let entry = BotLaunch {
        id: bot_launch_id(&bot.id),
        bot_id: bot.id.clone(),
        name: bot.name.clone(),
        agent_id: None,
        agent_name: None,
        blocked_reason: None,
    };
    match resolve(bot, workspace_agent, catalog) {
        Err(error) => BotLaunch { blocked_reason: Some(unavailable_message(&error)), ..entry },
        Ok((agent, resolved)) => BotLaunch {
            agent_id: Some(resolved.agent_id),
            agent_name: Some(agent.display_name().to_string()),
            blocked_reason: agent
                .capabilities()
                .interactive_launch
                .is_none()
                .then(|| unsupported_message(agent.display_name())),
            ..entry
        },
    }
}

fn launch_skill(skill: &SkillRef) -> LaunchSkill<'_> {
    match skill {
        SkillRef::Plugin(id) => LaunchSkill::Plugin(id),
        SkillRef::User(name) => LaunchSkill::User(name),
    }
}

/// Il lancio del Bot e le variabili d'ambiente da preparare per il tab prima
/// che parta. `user_skill_exists` dice se una skill utente c'e' ancora: se manca
/// il lancio fallisce, senza ripiegare su un Bot senza skill.
pub fn build_bot_launch(
    bot: &AgentBot,
    workspace_agent: &str,
    catalog: &AgentCatalog,
    user_skill_exists: impl Fn(&str) -> bool,
) -> Result<(BotTabLaunch, Vec<(String, String)>), String> {
    if !bot.enabled {
        return Err(format!("Il Bot «{}» è disabilitato.", bot.name));
    }
    let (agent, resolved) = resolve(bot, workspace_agent, catalog).map_err(|e| unavailable_message(&e))?;
    let launcher: &dyn InteractiveLauncher =
        agent.capabilities().interactive_launch.ok_or_else(|| unsupported_message(agent.display_name()))?;
    if let Some(SkillRef::User(name)) = &resolved.skill {
        if !user_skill_exists(name) {
            return Err(format!(
                "La skill «{name}» del Bot «{}» non esiste più: sceglila di nuovo nelle impostazioni del Bot.",
                bot.name
            ));
        }
    }
    let request = InteractiveRequest {
        model: resolved.model.as_deref(),
        effort: resolved.effort,
        skill: resolved.skill.as_ref().map(launch_skill),
    };
    let launch = launcher.launch(&request).ok_or_else(|| {
        format!(
            "Il Bot «{}» ha un modello o una skill con caratteri non ammessi nella riga di comando: correggilo nelle impostazioni.",
            bot.name
        )
    })?;
    if let Some(name) = first_invalid_env_name(&launch.env) {
        return Err(format!(
            "{} ha prodotto una variabile d'ambiente non valida ({name}) per il Bot «{}».",
            agent.display_name(),
            bot.name
        ));
    }
    let tab = BotTabLaunch {
        name: bot.name.clone(),
        agent_id: resolved.agent_id,
        command: launch.command,
        restore_command: launch.restore_command,
        warnings: resolved.warnings,
    };
    Ok((tab, launch.env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::{FakeIntegration, FAKE_MODEL_LARGE};
    use crate::bots::agent::ModelChoice;
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

    fn preset(id: &str) -> AgentBot {
        presets::find(id).unwrap().bot()
    }

    fn build(bot: &AgentBot, workspace: &str) -> Result<(BotTabLaunch, Vec<(String, String)>), String> {
        with_catalog(|c| build_bot_launch(bot, workspace, c, |name| name == "my-skill"))
    }

    #[test]
    fn only_enabled_bots_are_listed_in_order_with_the_prefix() {
        let off = AgentBot { enabled: false, ..preset("intake") };
        let bots = [preset(GENERAL_ID), off, preset("setup")];
        let list = with_catalog(|c| bot_launches(&bots, FULL, c));
        let ids: Vec<_> = list.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["bot:general", "bot:setup"]);
        assert_eq!(list[0].bot_id, "general");
        assert_eq!(list[0].name, "General");
    }

    #[test]
    fn a_launchable_bot_shows_its_effective_agent() {
        let list = with_catalog(|c| bot_launches(&[preset(GENERAL_ID)], FULL, c));
        assert_eq!(
            list[0],
            BotLaunch {
                id: "bot:general".into(),
                bot_id: "general".into(),
                name: "General".into(),
                agent_id: Some(FULL.into()),
                agent_name: Some("Fake".into()),
                blocked_reason: None,
            }
        );
    }

    #[test]
    fn an_unusable_agent_blocks_the_entry_in_italian() {
        let disabled = AgentBot { agent: Some(OFF.into()), ..preset(GENERAL_ID) };
        let unknown = AgentBot { agent: Some("gone".into()), ..preset("setup") };
        let list = with_catalog(|c| bot_launches(&[disabled, unknown], FULL, c));
        assert_eq!(list[0].blocked_reason.as_deref(), Some("Fake è disabilitato: abilitalo nelle Integrazioni."));
        assert_eq!(list[1].blocked_reason.as_deref(), Some("L'agente «gone» del Bot non è registrato."));
        for entry in &list {
            assert_eq!((&entry.agent_id, &entry.agent_name), (&None, &None));
        }
    }

    #[test]
    fn an_agent_without_the_contract_blocks_but_keeps_its_identity() {
        let list = with_catalog(|c| bot_launches(&[preset(GENERAL_ID)], BARE, c));
        assert_eq!(list[0].blocked_reason.as_deref(), Some("Fake non supporta l'avvio dei Bot."));
        assert_eq!(list[0].agent_id.as_deref(), Some(BARE));
        assert_eq!(list[0].agent_name.as_deref(), Some("Fake"));
    }

    #[test]
    fn a_bot_becomes_the_command_of_its_agent_with_model_effort_and_skill() {
        let (tab, env) = build(&preset("intake"), FULL).unwrap();
        assert_eq!(tab.name, "Intake");
        assert_eq!(tab.agent_id, FULL);
        assert_eq!(
            tab.command,
            format!("fake --yes --model {FAKE_MODEL_LARGE} --think high \"/fake-plugin:task-intake\"")
        );
        assert_eq!(tab.restore_command, format!("fake --yes --model {FAKE_MODEL_LARGE} --think high"));
        assert!(tab.warnings.is_empty() && env.is_empty());
    }

    #[test]
    fn what_the_agent_cannot_apply_comes_back_as_warnings() {
        let (tab, _) = build(&AgentBot { skill: None, ..preset("setup") }, FULL).unwrap();
        assert_eq!(tab.warnings, ["Fake does not support the effort level \"medium\": ignored"]);
        assert!(!tab.command.contains("--think"));
    }

    #[test]
    fn a_missing_user_skill_fails_instead_of_falling_back() {
        let bot = |name: &str| AgentBot { skill: Some(SkillRef::User(name.into())), ..preset(GENERAL_ID) };
        let (tab, _) = build(&bot("my-skill"), FULL).unwrap();
        assert_eq!(tab.command, "fake --yes \"/my-skill\"");
        assert_eq!(
            build(&bot("gone"), FULL).err().as_deref(),
            Some("La skill «gone» del Bot «General» non esiste più: sceglila di nuovo nelle impostazioni del Bot.")
        );
    }

    #[test]
    fn unsafe_skill_names_are_refused_with_a_message_for_the_user() {
        let bot = AgentBot { skill: Some(SkillRef::User("a b".into())), ..preset(GENERAL_ID) };
        let error = with_catalog(|c| build_bot_launch(&bot, FULL, c, |_| true)).err().unwrap();
        assert!(error.contains("caratteri non ammessi"), "{error}");
    }

    #[test]
    fn a_model_the_agent_knows_is_passed_through() {
        let bot =
            AgentBot { agent: Some(FULL.into()), model: ModelChoice::Id("fake-small".into()), ..preset(GENERAL_ID) };
        assert_eq!(build(&bot, BARE).unwrap().0.command, "fake --yes --model fake-small");
    }

    #[test]
    fn disabled_bots_and_unusable_agents_do_not_launch() {
        let off = AgentBot { enabled: false, ..preset(GENERAL_ID) };
        assert_eq!(build(&off, FULL).err().as_deref(), Some("Il Bot «General» è disabilitato."));
        assert_eq!(
            build(&preset(GENERAL_ID), "gone").err().as_deref(),
            Some("L'agente «gone» del Bot non è registrato.")
        );
        assert_eq!(build(&preset(GENERAL_ID), BARE).err().as_deref(), Some("Fake non supporta l'avvio dei Bot."));
        let disabled = AgentBot { agent: Some(OFF.into()), ..preset(GENERAL_ID) };
        assert_eq!(build(&disabled, FULL).err().as_deref(), Some("Fake è disabilitato: abilitalo nelle Integrazioni."));
    }
}
