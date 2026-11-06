//! I Bot come li vede un segretario (puro): l'elenco nel system prompt di chi
//! apre o modifica lavoro sulla board (`open_plan_task`, `request_task_update`)
//! e la verifica del Bot scelto, prima di toccare la board. Testi in inglese:
//! li legge il modello. Il resto della risoluzione (agente, modello, skill) e'
//! della board (`board::resolve_spec`).

use crate::bots::AgentBot;
use crate::hub::settings::{Secretary, SecretaryTool};
use crate::hub::text::clean_line;

/// Caratteri tenuti del nome di un Bot nel contesto del modello (`bots::validate` ne ammette 24).
const NAME_KEPT: usize = 40;
/// Caratteri tenuti del compito di un Bot (`bots::validate` ne ammette 300).
const TASK_KEPT: usize = 320;
/// Caratteri tenuti dell'id scelto dal modello, nei messaggi d'errore.
const ID_KEPT: usize = 64;

const INTRO: &str = "Bots: every planning task you open (`open_plan_task`) runs with a bot, and a task's bot can be \
changed with `request_task_update`. A bot fixes the coding agent, the model, the effort and the skill of the task. \
Choose the bot whose task fits the work; use `general` when no bot is specific. The available bots \
(id: name - task):";
const NO_BOTS: &str = "Bots: no bot is available now, so you cannot open planning tasks: tell the user to enable a \
bot in the settings.";

/// La sezione dei Bot del system prompt, solo per un segretario che usa un tool
/// che li chiede; i Bot disabilitati non si elencano.
pub fn describe(secretary: &Secretary, bots: &[AgentBot]) -> Option<String> {
    let needs_bots =
        [SecretaryTool::OpenPlanTask, SecretaryTool::RequestTaskUpdate].into_iter().any(|t| secretary.may_use(t));
    if !needs_bots {
        return None;
    }
    let lines: Vec<String> = bots
        .iter()
        .filter(|b| b.enabled)
        .map(|b| format!("- {}: {} - {}", b.id, clean_line(&b.name, NAME_KEPT), clean_line(&b.task, TASK_KEPT)))
        .collect();
    if lines.is_empty() {
        return Some(NO_BOTS.to_string());
    }
    Some(format!("{INTRO}\n{}", lines.join("\n")))
}

/// Il Bot `id` esiste ed e' abilitato; altrimenti l'errore per il modello,
/// senza ripieghi su un altro Bot.
pub fn check(bots: &[AgentBot], id: &str) -> Result<(), String> {
    let shown = clean_line(id, ID_KEPT);
    match bots.iter().find(|b| b.id == id) {
        None => Err(format!(
            "Unknown bot \"{shown}\": use the id of one of the bots listed in your instructions (`general` when no \
            bot is specific)."
        )),
        Some(bot) if !bot.enabled => Err(format!(
            "The bot \"{shown}\" is disabled: choose another of the bots listed in your instructions, or ask the \
            user to enable it in the settings."
        )),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::default_agent_bots;
    use crate::hub::engine::fixtures::secretary;

    fn bots() -> Vec<AgentBot> {
        default_agent_bots()
    }

    fn with_tools(tools: Vec<SecretaryTool>) -> Secretary {
        Secretary { tools, ..secretary("mario") }
    }

    #[test]
    fn the_enabled_bots_are_listed_with_id_name_and_task() {
        let mut list = bots();
        list[1].enabled = false;
        let text = describe(&with_tools(vec![SecretaryTool::OpenPlanTask]), &list).unwrap();
        assert!(text.starts_with(INTRO), "{text}");
        let general = &list[0];
        assert!(text.contains(&format!("\n- general: {} - {}", general.name, general.task)), "{text}");
        assert!(!text.contains(&format!("- {}:", list[1].id)), "un Bot disabilitato non si elenca: {text}");
        assert!(text.contains(&format!("- {}:", list[2].id)), "{text}");
    }

    #[test]
    fn only_a_secretary_that_opens_or_changes_work_sees_the_bots() {
        let update = with_tools(vec![SecretaryTool::RequestTaskUpdate]);
        assert!(describe(&update, &bots()).is_some());
        let reader = with_tools(vec![SecretaryTool::BoardStatus, SecretaryTool::PostMessage]);
        assert_eq!(describe(&reader, &bots()), None);
    }

    #[test]
    fn without_enabled_bots_the_secretary_is_told_so() {
        let mut list = bots();
        list.iter_mut().for_each(|b| b.enabled = false);
        assert_eq!(describe(&with_tools(vec![SecretaryTool::OpenPlanTask]), &list).as_deref(), Some(NO_BOTS));
    }

    #[test]
    fn an_unknown_or_disabled_bot_is_refused_without_falling_back() {
        let mut list = bots();
        assert_eq!(check(&list, "general"), Ok(()));
        let unknown = check(&list, "nope").unwrap_err();
        assert!(unknown.starts_with("Unknown bot \"nope\""), "{unknown}");
        let id = list[1].id.clone();
        list[1].enabled = false;
        let disabled = check(&list, &id).unwrap_err();
        assert!(disabled.starts_with(&format!("The bot \"{id}\" is disabled")), "{disabled}");
    }
}
