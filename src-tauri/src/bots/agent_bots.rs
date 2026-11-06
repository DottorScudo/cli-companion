//! Operazioni pure sull'elenco dei Bot-agente: creare, modificare, eliminare,
//! ripristinare un preset o ricreare quelli mancanti. Un'operazione che fallisce
//! non cambia nulla. I campi arrivano gia' validati (`validate::validate_draft`);
//! qui si controllano unicita' di id e nomi e le regole del Bot `general`.
//! `taken` sono i nomi che un Bot non puo' prendere perche' gia' usati altrove
//! (i segretari dell'HUB), in minuscolo.

use super::agent::{AgentBot, AgentBotDraft};
use super::presets::{self, GENERAL_ID, PRESETS, USER_ID_PREFIX};
use super::validate::NAME_MAX_CHARS;

/// Bot creati al massimo (i preset ricreati non contano per il tetto).
pub const AGENT_BOTS_MAX: usize = 50;

/// Lunghezza massima della parte dell'id ricavata dal nome.
const ID_SLUG_MAX: usize = 32;
/// Parte dell'id quando il nome non ha lettere o cifre ASCII.
const ID_FALLBACK: &str = "bot";

const NOT_FOUND: &str = "Bot non trovato";
const NOT_FROM_PRESET: &str = "questo Bot non nasce da un predefinito";
const GENERAL_UNDELETABLE: &str = "il Bot generico non si può eliminare";
const GENERAL_ALWAYS_ON: &str = "il Bot generico non si può disattivare";

/// Aggiunge un Bot in fondo e ne restituisce l'id.
pub fn create(list: &mut Vec<AgentBot>, draft: AgentBotDraft, taken: &[String]) -> Result<String, String> {
    if list.len() >= AGENT_BOTS_MAX {
        return Err(format!("al massimo {AGENT_BOTS_MAX} Bot"));
    }
    ensure_free(list, &draft.name, None, taken)?;
    let id = new_id(list, &draft.name);
    list.push(AgentBot {
        id: id.clone(),
        name: draft.name,
        task: draft.task,
        agent: draft.agent,
        model: draft.model,
        effort: draft.effort,
        skill: draft.skill,
        kinds: draft.kinds,
        enabled: draft.enabled,
        image: draft.image,
        preset: None,
    });
    Ok(id)
}

/// Sostituisce i campi del Bot `id`; id e preset d'origine restano.
pub fn update(list: &mut [AgentBot], id: &str, draft: AgentBotDraft, taken: &[String]) -> Result<(), String> {
    let at = position(list, id)?;
    if id == GENERAL_ID && !draft.enabled {
        return Err(GENERAL_ALWAYS_ON.into());
    }
    ensure_free(list, &draft.name, Some(at), taken)?;
    let bot = &mut list[at];
    bot.name = draft.name;
    bot.task = draft.task;
    bot.agent = draft.agent;
    bot.model = draft.model;
    bot.effort = draft.effort;
    bot.skill = draft.skill;
    bot.kinds = draft.kinds;
    bot.enabled = draft.enabled;
    bot.image = draft.image;
    Ok(())
}

pub fn delete(list: &mut Vec<AgentBot>, id: &str) -> Result<(), String> {
    if id == GENERAL_ID {
        return Err(GENERAL_UNDELETABLE.into());
    }
    let at = position(list, id)?;
    list.remove(at);
    Ok(())
}

/// Riporta compito, modello, effort, skill e tipi di task del Bot `id` a quelli
/// del suo preset; nome, immagine, agente e accensione restano.
pub fn restore_preset(list: &mut [AgentBot], id: &str) -> Result<(), String> {
    let at = position(list, id)?;
    let preset_id = list[at].preset.as_deref().ok_or(NOT_FROM_PRESET)?;
    let preset = presets::find(preset_id).ok_or(NOT_FROM_PRESET)?;
    let bot = &mut list[at];
    bot.task = preset.task.to_string();
    bot.model = preset.model.clone();
    bot.effort = preset.effort;
    bot.skill = preset.skill();
    bot.kinds = preset.kinds.to_vec();
    Ok(())
}

/// Ricrea in fondo i preset che mancano; quelli presenti non cambiano. Se il
/// nome del preset e' gia' preso, prende il primo libero fra "Nome 2", "Nome 3"...
pub fn restore_presets(list: &mut Vec<AgentBot>, taken: &[String]) {
    for preset in &PRESETS {
        if list.iter().any(|b| b.id == preset.id) {
            continue;
        }
        let mut bot = preset.bot();
        bot.name = free_name(list, preset.name, taken);
        list.push(bot);
    }
}

fn position(list: &[AgentBot], id: &str) -> Result<usize, String> {
    list.iter().position(|b| b.id == id).ok_or_else(|| NOT_FOUND.to_string())
}

/// Il nome non e' di un altro Bot (`except` = chi viene modificato) ne' in `taken`.
fn ensure_free(list: &[AgentBot], name: &str, except: Option<usize>, taken: &[String]) -> Result<(), String> {
    let wanted = name.to_lowercase();
    if list.iter().enumerate().any(|(i, b)| Some(i) != except && b.name.to_lowercase() == wanted) {
        return Err(format!("c'è già un Bot di nome “{name}”"));
    }
    if taken.contains(&wanted) {
        return Err(format!("“{name}” è già il nome di un segretario"));
    }
    Ok(())
}

fn is_free(list: &[AgentBot], name: &str, taken: &[String]) -> bool {
    ensure_free(list, name, None, taken).is_ok()
}

/// `base`, o `base N` col primo N libero, entro la lunghezza massima dei nomi.
pub(super) fn free_name(list: &[AgentBot], base: &str, taken: &[String]) -> String {
    if is_free(list, base, taken) {
        return base.to_string();
    }
    (2..)
        .map(|n| {
            let suffix = format!(" {n}");
            let room = NAME_MAX_CHARS.saturating_sub(suffix.chars().count());
            format!("{}{suffix}", base.chars().take(room).collect::<String>())
        })
        .find(|name| is_free(list, name, taken))
        .expect("i suffissi numerici sono infiniti")
}

/// `custom-<slug del nome>`, con un numero in coda se gia' usato.
fn new_id(list: &[AgentBot], name: &str) -> String {
    let base = format!("{USER_ID_PREFIX}{}", slug(name));
    let used = |id: &str| list.iter().any(|b| b.id == id);
    if !used(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !used(id)).expect("i suffissi numerici sono infiniti")
}

/// Lettere e cifre ASCII in minuscolo, il resto diventa un solo `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_end_matches('-').chars().take(ID_SLUG_MAX).collect();
    let trimmed = trimmed.trim_end_matches('-');
    if trimmed.is_empty() {
        ID_FALLBACK.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Effort, ModelTier};
    use crate::board::TaskKind;
    use crate::bots::agent::{ModelChoice, SkillRef};
    use crate::bots::presets::default_bots;
    use crate::bots::BotImage;

    fn draft(name: &str) -> AgentBotDraft {
        AgentBotDraft {
            name: name.into(),
            task: format!("{name} task."),
            agent: None,
            model: ModelChoice::Default,
            effort: None,
            skill: None,
            kinds: vec![TaskKind::Work],
            enabled: true,
            image: None,
        }
    }

    fn ids(list: &[AgentBot]) -> Vec<&str> {
        list.iter().map(|b| b.id.as_str()).collect()
    }

    fn by_id<'a>(list: &'a [AgentBot], id: &str) -> &'a AgentBot {
        list.iter().find(|b| b.id == id).unwrap()
    }

    #[test]
    fn create_appends_with_a_custom_id_and_no_preset() {
        let mut list = default_bots();
        let id = create(&mut list, draft("Code Reviewer"), &[]).unwrap();
        assert_eq!(id, "custom-code-reviewer");
        let bot = list.last().unwrap();
        assert_eq!((bot.id.as_str(), bot.name.as_str(), bot.preset.as_ref()), (id.as_str(), "Code Reviewer", None));
    }

    #[test]
    fn ids_are_stable_slugs_and_unique() {
        assert_eq!(slug("  Héllo, World!! "), "h-llo-world");
        assert_eq!(slug("ñ ñ"), ID_FALLBACK);
        assert_eq!(slug(&"ab".repeat(40)).len(), ID_SLUG_MAX);
        let mut list = Vec::new();
        assert_eq!(create(&mut list, draft("A b"), &[]).unwrap(), "custom-a-b");
        assert_eq!(create(&mut list, draft("A-b"), &[]).unwrap(), "custom-a-b-2");
        assert_eq!(create(&mut list, draft("A_b"), &[]).unwrap(), "custom-a-b-3");
    }

    #[test]
    fn names_are_unique_ignoring_case_and_not_secretary_names() {
        let mut list = default_bots();
        let before = list.clone();
        assert!(create(&mut list, draft("intake"), &[]).unwrap_err().contains("già un Bot"));
        let taken = vec!["mario".to_string()];
        assert!(create(&mut list, draft("Mario"), &taken).unwrap_err().contains("segretario"));
        assert_eq!(list, before);
        // Rinominare se stesso cambiando solo le maiuscole e' ammesso.
        update(&mut list, "intake", draft("INTAKE"), &taken).unwrap();
        assert_eq!(by_id(&list, "intake").name, "INTAKE");
        assert!(update(&mut list, "intake", draft("setup"), &taken).is_err());
        assert!(update(&mut list, "intake", draft("mario"), &taken).is_err());
    }

    #[test]
    fn the_number_of_bots_is_capped() {
        let mut list: Vec<AgentBot> = Vec::new();
        for i in 0..AGENT_BOTS_MAX {
            create(&mut list, draft(&format!("Bot {i}")), &[]).unwrap();
        }
        assert!(create(&mut list, draft("Extra"), &[]).is_err());
        assert_eq!(list.len(), AGENT_BOTS_MAX);
    }

    #[test]
    fn update_replaces_the_fields_and_keeps_id_and_preset() {
        let mut list = default_bots();
        let image = BotImage { png: "AAAA".into(), color: "#112233".into() };
        let next = AgentBotDraft {
            agent: Some("fake".into()),
            model: ModelChoice::Id("m".into()),
            effort: Some(Effort::Low),
            skill: Some(SkillRef::User("mine".into())),
            enabled: false,
            image: Some(image.clone()),
            ..draft("Organizer")
        };
        update(&mut list, "intake", next, &[]).unwrap();
        let bot = by_id(&list, "intake");
        assert_eq!((bot.name.as_str(), bot.preset.as_deref(), bot.enabled), ("Organizer", Some("intake"), false));
        assert_eq!((bot.model.clone(), bot.image.clone()), (ModelChoice::Id("m".into()), Some(image)));
        assert_eq!(bot.kinds, [TaskKind::Work], "the kinds come from the draft");
        assert_eq!(update(&mut list, "nope", draft("X"), &[]).unwrap_err(), NOT_FOUND);
    }

    #[test]
    fn general_can_be_configured_but_never_disabled_or_deleted() {
        let mut list = default_bots();
        update(&mut list, GENERAL_ID, draft("Generic"), &[]).unwrap();
        assert_eq!(by_id(&list, GENERAL_ID).name, "Generic");
        let off = AgentBotDraft { enabled: false, ..draft("Generic") };
        assert_eq!(update(&mut list, GENERAL_ID, off, &[]).unwrap_err(), GENERAL_ALWAYS_ON);
        assert_eq!(delete(&mut list, GENERAL_ID).unwrap_err(), GENERAL_UNDELETABLE);
        assert!(by_id(&list, GENERAL_ID).enabled);
    }

    #[test]
    fn delete_removes_only_that_bot() {
        let mut list = default_bots();
        delete(&mut list, "setup").unwrap();
        assert_eq!(
            ids(&list),
            [
                "general",
                "intake",
                "skill-wizard",
                "reviewer",
                "planner",
                "quick-answer",
                "bug-fixer",
                "researcher",
                "test-writer",
                "docs-writer",
                "refactorer"
            ]
        );
        assert_eq!(delete(&mut list, "setup").unwrap_err(), NOT_FOUND);
    }

    #[test]
    fn restoring_a_preset_keeps_name_image_agent_and_enabled() {
        let mut list = default_bots();
        let image = BotImage { png: "AAAA".into(), color: "#112233".into() };
        let changed = AgentBotDraft {
            task: "Something else.".into(),
            agent: Some("fake".into()),
            model: ModelChoice::Default,
            effort: Some(Effort::Max),
            skill: None,
            enabled: false,
            image: Some(image.clone()),
            ..draft("Organizer")
        };
        update(&mut list, "intake", changed, &[]).unwrap();
        restore_preset(&mut list, "intake").unwrap();
        let bot = by_id(&list, "intake");
        assert_eq!(bot.task, presets::find("intake").unwrap().task);
        assert_eq!(bot.model, ModelChoice::Tier(ModelTier::Strong));
        assert_eq!(bot.effort, Some(Effort::High));
        assert_eq!(bot.skill, Some(SkillRef::Plugin("task-intake".into())));
        assert_eq!((bot.name.as_str(), bot.image.clone()), ("Organizer", Some(image)));
        assert_eq!((bot.agent.as_deref(), bot.enabled), (Some("fake"), false));
        assert!(bot.kinds.is_empty(), "the kinds of the preset come back");
    }

    #[test]
    fn only_preset_bots_can_be_restored() {
        let mut list = default_bots();
        let id = create(&mut list, draft("Mine"), &[]).unwrap();
        assert_eq!(restore_preset(&mut list, &id).unwrap_err(), NOT_FROM_PRESET);
        assert_eq!(restore_preset(&mut list, "nope").unwrap_err(), NOT_FOUND);
        list[1].preset = Some("retired".into());
        let retired = list[1].id.clone();
        assert_eq!(restore_preset(&mut list, &retired).unwrap_err(), NOT_FROM_PRESET);
    }

    #[test]
    fn restoring_all_presets_recreates_only_the_missing_ones() {
        let mut list = default_bots();
        update(&mut list, "intake", draft("Organizer"), &[]).unwrap();
        delete(&mut list, "setup").unwrap();
        delete(&mut list, "skill-wizard").unwrap();
        restore_presets(&mut list, &[]);
        assert_eq!(
            ids(&list),
            [
                "general",
                "intake",
                "reviewer",
                "planner",
                "quick-answer",
                "bug-fixer",
                "researcher",
                "test-writer",
                "docs-writer",
                "refactorer",
                "setup",
                "skill-wizard"
            ]
        );
        assert_eq!(by_id(&list, "intake").name, "Organizer");
        assert_eq!(by_id(&list, "setup"), &presets::find("setup").unwrap().bot());
        let again = list.clone();
        restore_presets(&mut list, &[]);
        assert_eq!(list, again);
    }

    #[test]
    fn a_recreated_preset_gets_a_free_name() {
        let mut list = default_bots();
        delete(&mut list, "setup").unwrap();
        create(&mut list, draft("Setup"), &[]).unwrap();
        restore_presets(&mut list, &["setup 2".to_string()]);
        assert_eq!(by_id(&list, "setup").name, "Setup 3");
        let long = "x".repeat(NAME_MAX_CHARS);
        let named = free_name(&[AgentBot { name: long.clone(), ..default_bots()[0].clone() }], &long, &[]);
        assert_eq!(named.chars().count(), NAME_MAX_CHARS);
        assert!(named.ends_with(" 2"));
    }
}
