//! Le vecchie voci di lancio (preset e orchestratore) e i loro Bot. Congelato
//! come un passo di migrazione: descrive gli id delle versioni che li avevano e
//! non cambia quando cambiano i Bot o le skill correnti.

use crate::bots::{bot_launch_id, BOT_LAUNCH_PREFIX};
use serde_json::Value;

/// Vecchia voce rapida dell'orchestratore (v0.11), poi diventata Task Intake.
const LEGACY_ORCHESTRATOR_LAUNCH: &str = "builtin:orchestrator";

/// Prefisso delle vecchie voci dei preset: `builtin:preset:<preset>[@<agente>]`.
const LEGACY_PRESET_PREFIX: &str = "builtin:preset:";

/// Separa il preset dall'agente scelto nelle vecchie voci.
const LEGACY_AGENT_SEPARATOR: char = '@';

/// Separa il Bot predefinito dall'agente nell'id di un Bot derivato.
const DERIVED_ID_SEPARATOR: char = '-';

/// Lunghezza massima dell'id di un agente letto da una vecchia voce.
const AGENT_ID_MAX: usize = 64;

/// Il preset di allora -> il Bot predefinito che lo sostituisce.
const PRESET_BOTS: [(&str, &str); 3] =
    [("task-intake", "intake"), ("project-setup", "setup"), ("skill-wizard", "skill-wizard")];

/// Il Bot dell'orchestratore, come quello del preset Task Intake.
const ORCHESTRATOR_BOT: &str = "intake";

/// Dove porta una vecchia voce di lancio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyTarget {
    /// Il Bot predefinito del preset, che segue l'agente del workspace.
    Bot(&'static str),
    /// Il Bot derivato dal predefinito `bot` con l'agente `agent` fissato.
    Derived { bot: &'static str, agent: String },
}

impl LegacyTarget {
    /// L'id del Bot di destinazione.
    pub fn bot_id(&self) -> String {
        match self {
            LegacyTarget::Bot(bot) => bot.to_string(),
            LegacyTarget::Derived { bot, agent } => derived_bot_id(bot, agent),
        }
    }

    /// La voce di lancio del Bot di destinazione.
    pub fn launch_id(&self) -> String {
        bot_launch_id(&self.bot_id())
    }

    /// La voce del Bot predefinito, senza l'agente scelto.
    pub fn base_launch_id(&self) -> String {
        match self {
            LegacyTarget::Bot(bot) | LegacyTarget::Derived { bot, .. } => bot_launch_id(bot),
        }
    }
}

/// L'id del Bot di una voce `bot:<id>`.
pub fn launch_bot_id(launch_id: &str) -> Option<&str> {
    launch_id.strip_prefix(BOT_LAUNCH_PREFIX)
}

/// `<bot>-<agente>`: l'id del Bot derivato. I Bot dell'utente hanno il loro
/// prefisso, quindi non lo possono avere.
pub fn derived_bot_id(bot: &str, agent: &str) -> String {
    format!("{bot}{DERIVED_ID_SEPARATOR}{agent}")
}

/// Il Bot predefinito e l'agente di un id di Bot derivato.
pub fn parse_derived_bot_id(bot_id: &str) -> Option<(&'static str, &str)> {
    PRESET_BOTS.iter().find_map(|&(_, bot)| {
        let agent = bot_id.strip_prefix(bot)?.strip_prefix(DERIVED_ID_SEPARATOR)?;
        is_agent_id(agent).then_some((bot, agent))
    })
}

/// Slug di un agente: minuscole, cifre e `-`, non vuoto ne' troppo lungo.
fn is_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= AGENT_ID_MAX
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Dove porta `id`, se e' una vecchia voce nota. Un agente con caratteri non
/// ammessi non diventa un id: si ripiega sul Bot predefinito.
pub fn parse_legacy(id: &str) -> Option<LegacyTarget> {
    if id == LEGACY_ORCHESTRATOR_LAUNCH {
        return Some(LegacyTarget::Bot(ORCHESTRATOR_BOT));
    }
    let rest = id.strip_prefix(LEGACY_PRESET_PREFIX)?;
    let (preset, agent) = match rest.split_once(LEGACY_AGENT_SEPARATOR) {
        Some((preset, agent)) => (preset, Some(agent)),
        None => (rest, None),
    };
    let bot = PRESET_BOTS.iter().find_map(|&(p, bot)| (p == preset).then_some(bot))?;
    Some(match agent.filter(|a| is_agent_id(a)) {
        Some(agent) => LegacyTarget::Derived { bot, agent: agent.to_string() },
        None => LegacyTarget::Bot(bot),
    })
}

/// Riscrive una vecchia voce nella voce del suo Bot; gli altri id restano.
pub fn migrate_launch_id(id: &mut Option<String>) {
    if let Some(target) = id.as_deref().and_then(parse_legacy) {
        *id = Some(target.launch_id());
    }
}

/// Come `migrate_launch_id`, su un valore JSON (solo se e' un testo).
pub fn migrate_launch_value(slot: &mut Value) {
    let mut id = slot.as_str().map(str::to_string);
    migrate_launch_id(&mut id);
    if let Some(id) = id {
        *slot = Value::String(id);
    }
}

/// Per un id letto dopo il caricamento (es. una proposta salvata prima
/// dell'aggiornamento): la voce del suo Bot, o del predefinito se il Bot
/// derivato non c'e'. Gli altri id restano.
pub fn current_launch_id(id: &str, bot_exists: impl Fn(&str) -> bool) -> String {
    match parse_legacy(id) {
        Some(target) if bot_exists(&target.bot_id()) => target.launch_id(),
        Some(target) => target.base_launch_id(),
        None => id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn migrated(id: &str) -> Option<String> {
        let mut id = Some(id.to_string());
        migrate_launch_id(&mut id);
        id
    }

    #[test]
    fn each_preset_goes_to_its_bot() {
        assert_eq!(migrated("builtin:preset:task-intake").as_deref(), Some("bot:intake"));
        assert_eq!(migrated("builtin:preset:project-setup").as_deref(), Some("bot:setup"));
        assert_eq!(migrated("builtin:preset:skill-wizard").as_deref(), Some("bot:skill-wizard"));
        assert_eq!(migrated("builtin:orchestrator").as_deref(), Some("bot:intake"));
    }

    #[test]
    fn a_chosen_agent_goes_to_a_derived_bot() {
        assert_eq!(migrated("builtin:preset:task-intake@opencode").as_deref(), Some("bot:intake-opencode"));
        assert_eq!(migrated("builtin:preset:skill-wizard@my-agent").as_deref(), Some("bot:skill-wizard-my-agent"));
        assert_eq!(
            parse_legacy("builtin:preset:project-setup@x1"),
            Some(LegacyTarget::Derived { bot: "setup", agent: "x1".into() })
        );
    }

    #[test]
    fn a_malformed_agent_falls_back_to_the_preset_bot() {
        for id in ["builtin:preset:task-intake@", "builtin:preset:task-intake@A b", "builtin:preset:task-intake@a/../b"]
        {
            assert_eq!(migrated(id).as_deref(), Some("bot:intake"), "{id}");
        }
        assert_eq!(migrated(&format!("builtin:preset:task-intake@{}", "a".repeat(65))).as_deref(), Some("bot:intake"));
    }

    #[test]
    fn other_ids_are_untouched() {
        for id in ["shell", "builtin:claude-code", "bot:general", "ln-1-1", "builtin:preset:gone", "builtin:preset:"] {
            assert_eq!(migrated(id).as_deref(), Some(id), "{id}");
        }
        let mut none = None;
        migrate_launch_id(&mut none);
        assert_eq!(none, None);
    }

    #[test]
    fn only_a_text_value_is_rewritten() {
        let cases = [
            (json!("builtin:preset:project-setup"), json!("bot:setup")),
            (json!("builtin:preset:setup"), json!("builtin:preset:setup")),
            (json!(null), json!(null)),
            (json!(3), json!(3)),
        ];
        for (mut value, expected) in cases {
            migrate_launch_value(&mut value);
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn derived_ids_round_trip_only_for_preset_bots() {
        assert_eq!(parse_derived_bot_id(&derived_bot_id("intake", "opencode")), Some(("intake", "opencode")));
        assert_eq!(parse_derived_bot_id("skill-wizard-claude-code"), Some(("skill-wizard", "claude-code")));
        for id in ["intake", "intake-", "general-opencode", "custom-intake-x", "setupx-y", "intake-A"] {
            assert_eq!(parse_derived_bot_id(id), None, "{id}");
        }
    }

    #[test]
    fn a_late_legacy_id_uses_the_derived_bot_only_if_it_exists() {
        let id = "builtin:preset:task-intake@opencode";
        assert_eq!(current_launch_id(id, |b| b == "intake-opencode"), "bot:intake-opencode");
        assert_eq!(current_launch_id(id, |_| false), "bot:intake");
        assert_eq!(current_launch_id("builtin:preset:skill-wizard", |_| false), "bot:skill-wizard");
        assert_eq!(current_launch_id("builtin:claude-code", |_| true), "builtin:claude-code");
    }

    #[test]
    fn the_bot_of_a_launch_id() {
        assert_eq!(launch_bot_id("bot:general"), Some("general"));
        assert_eq!(launch_bot_id("builtin:x"), None);
    }
}
